mod plan;
mod policy;
mod pty;
mod store;
mod trust;
mod worktree;

use std::path::PathBuf;
use std::sync::Arc;

use plan::{Plan, PlanStore};
use policy::Program;
use pty::{PtyEvent, PtyManager};
use serde::Serialize;
use store::{Store, Workspace};
use tauri::ipc::Channel;
use tauri::{Manager, State};

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum PtyMessage {
    Output { data: Vec<u8> },
    Exited { code: Option<u32> },
}

fn valid_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err("invalid session id".into())
    }
}

fn channel_sink(on_event: Channel<PtyMessage>) -> pty::EventSink {
    Arc::new(move |_id: &str, ev: PtyEvent| {
        let msg = match ev {
            PtyEvent::Output(data) => PtyMessage::Output { data },
            PtyEvent::Exited(code) => PtyMessage::Exited { code },
        };
        if let Err(e) = on_event.send(msg) {
            eprintln!("pty channel send failed: {e}");
        }
    })
}

/// Opens a shell session. The program is fixed by the backend; the frontend never chooses it.
#[tauri::command]
fn pty_spawn_shell(
    pty: State<'_, PtyManager>,
    store: State<'_, Store>,
    workspace_id: i64,
    id: String,
    cols: u16,
    rows: u16,
    on_event: Channel<PtyMessage>,
) -> Result<(), String> {
    valid_id(&id)?;
    let root = store
        .get_workspace(workspace_id)
        .map_err(|e| e.to_string())?
        .root();
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let mut spec = policy::authorize(
        Program::Shell,
        vec!["-NoLogo".into()],
        &root,
        &root,
        pty.count(),
        &path_var,
    )
    .map_err(|e| e.to_string())?;
    spec.cols = cols.max(1);
    spec.rows = rows.max(1);
    pty.spawn(&id, spec, channel_sink(on_event))
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanDraft {
    id: u64,
    plan: Plan,
}

/// Ask the orchestrator (`claude -p`, no tools) for a team plan. The plan is stored unapproved.
#[tauri::command]
async fn plan_generate(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    workspace_id: i64,
    request: String,
) -> Result<PlanDraft, String> {
    let request = request.trim().to_string();
    if request.is_empty() || request.len() > plan::MAX_REQUEST_LEN {
        return Err("request is empty or too long".into());
    }
    let root = store
        .get_workspace(workspace_id)
        .map_err(|e| e.to_string())?
        .root();
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    // Session count 0: the planner is a short-lived child, not a PTY session.
    let spec = policy::authorize(
        Program::Claude,
        plan::planner_args(&request),
        &root,
        &root,
        0,
        &path_var,
    )
    .map_err(|e| e.to_string())?;
    let plan = tauri::async_runtime::spawn_blocking(move || plan::run_planner(&spec))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let id = plans.insert(workspace_id, plan.clone());
    Ok(PlanDraft { id, plan })
}

fn folder_trusted(root: &std::path::Path) -> Result<bool, String> {
    let root = policy::simplify(root.canonicalize().map_err(|e| e.to_string())?);
    trust::config_path()
        .and_then(|c| trust::is_trusted(&c, &root))
        .map_err(|e| e.to_string())
}

/// Whether Claude Code already trusts the workspace folder (read-only check).
#[tauri::command]
fn workspace_trusted(store: State<'_, Store>, workspace_id: i64) -> Result<bool, String> {
    let root = store
        .get_workspace(workspace_id)
        .map_err(|e| e.to_string())?
        .root();
    folder_trusted(&root)
}

/// Approve a plan: create one git worktree + branch per agent (all or nothing), then record it.
#[tauri::command]
async fn plan_approve(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<(), String> {
    let (workspace_id, plan) = plans.pending(plan_id).map_err(|e| e.to_string())?;
    let root = store
        .get_workspace(workspace_id)
        .map_err(|e| e.to_string())?
        .root();
    let root = policy::simplify(root.canonicalize().map_err(|e| e.to_string())?);
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let exe = policy::resolve_program(Program::Git, &path_var).map_err(|e| e.to_string())?;
    let env_remove = policy::stripped_env(std::env::vars_os().map(|(k, _)| k));
    let git = worktree::Git::new(exe, root, env_remove);
    // Run id: unique per approval and across restarts, so branch names never collide.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let run = format!("{secs:x}");
    let ids: Vec<String> = plan.agents.iter().map(|a| a.id.clone()).collect();
    let (git, team) = tauri::async_runtime::spawn_blocking(move || {
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let team = git.create_team(&run, &refs);
        (git, team)
    })
    .await
    .map_err(|e| e.to_string())?;
    let team = team.map_err(|e| e.to_string())?;
    let map = plan
        .agents
        .iter()
        .map(|a| a.id.clone())
        .zip(team.iter().cloned())
        .collect();
    plans.approve(plan_id, map).map_err(|e| {
        git.remove_team(&team);
        e.to_string()
    })
}

/// Opens an interactive `claude` session for the orchestrator or one agent of an approved plan.
/// The prompt is built here from the stored plan, never taken from the frontend.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn pty_spawn_agent(
    pty: State<'_, PtyManager>,
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
    agent_id: String,
    id: String,
    cols: u16,
    rows: u16,
    on_event: Channel<PtyMessage>,
) -> Result<(), String> {
    valid_id(&id)?;
    let team = plans.approved(plan_id).map_err(|e| e.to_string())?;
    let root = store
        .get_workspace(team.workspace_id)
        .map_err(|e| e.to_string())?
        .root();
    if !folder_trusted(&root)? {
        return Err(trust::NOT_TRUSTED.into());
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    // Orchestrator works in the workspace root; each agent in its own worktree.
    let (prompt, cwd) = if agent_id == plan::ORCHESTRATOR_ID {
        (
            plan::orchestrator_prompt(&team.plan, &team.worktrees),
            root.clone(),
        )
    } else {
        let agent = team.plan.agent(&agent_id).map_err(|e| e.to_string())?;
        let wt = team
            .worktrees
            .get(&agent_id)
            .ok_or("agent has no worktree")?;
        let prompt = plan::agent_prompt(&team.plan, agent, &wt.branch);
        (prompt, wt.path.clone())
    };
    let mut spec = policy::authorize(
        Program::Claude,
        vec![prompt],
        &root,
        &cwd,
        pty.count(),
        &path_var,
    )
    .map_err(|e| e.to_string())?;
    spec.cols = cols.max(1);
    spec.rows = rows.max(1);
    pty.spawn(&id, spec, channel_sink(on_event))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn pty_write(pty: State<'_, PtyManager>, id: String, data: String) -> Result<(), String> {
    pty.write(&id, data.as_bytes()).map_err(|e| e.to_string())
}

#[tauri::command]
fn pty_resize(pty: State<'_, PtyManager>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    pty.resize(&id, cols.max(1), rows.max(1))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn pty_kill(pty: State<'_, PtyManager>, id: String) -> Result<(), String> {
    pty.kill(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn workspace_list(store: State<'_, Store>) -> Result<Vec<Workspace>, String> {
    store.list_workspaces().map_err(|e| e.to_string())
}

#[tauri::command]
fn workspace_add(store: State<'_, Store>, path: String) -> Result<Workspace, String> {
    store
        .add_workspace(&PathBuf::from(path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn workspace_remove(store: State<'_, Store>, id: i64) -> Result<(), String> {
    store.remove_workspace(id).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(PtyManager::default())
        .manage(PlanStore::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(Store::open(&dir.join("ina.db"))?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pty_spawn_shell,
            pty_spawn_agent,
            plan_generate,
            plan_approve,
            pty_write,
            pty_resize,
            pty_kill,
            workspace_list,
            workspace_add,
            workspace_remove,
            workspace_trusted
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::valid_id;

    #[test]
    fn session_id_validation() {
        assert!(valid_id("agent-1_a").is_ok());
        for bad in ["", "../x", "a b", "a;b", &"x".repeat(65)] {
            assert!(valid_id(bad).is_err(), "{bad}");
        }
    }
}
