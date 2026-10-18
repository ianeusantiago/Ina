mod plan;
mod policy;
mod pty;
mod setup;
mod store;
mod tasks;
mod trust;
mod worktree;

use std::path::PathBuf;
use std::sync::Arc;

use plan::{Plan, PlanStore};
use policy::Program;
use pty::{PtyEvent, PtyManager};
use serde::Serialize;
use store::{Store, Workspace};
use tasks::{Registry, TaskStatus};
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
            PtyEvent::Exited { code, .. } => PtyMessage::Exited { code },
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
    let root = workspace_root(&store, workspace_id)?;
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

/// Approved, open teams of a workspace, so they can be resumed after a restart.
#[tauri::command]
fn team_list(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    workspace_id: i64,
) -> Result<Vec<PlanDraft>, String> {
    Ok(plans
        .open_teams(&store, workspace_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|t| PlanDraft {
            id: t.id,
            plan: t.plan,
        })
        .collect())
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
    let root = workspace_root(&store, workspace_id)?;
    // `-p` skips Claude Code's trust dialog but still loads project settings and hooks.
    if !folder_trusted(&root)? {
        return Err(trust::NOT_TRUSTED.into());
    }
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
    let plan = blocking(move || plan::run_planner(&spec).map_err(|e| e.to_string())).await?;
    let id = plans
        .insert(&store, workspace_id, &plan)
        .map_err(|e| e.to_string())?;
    Ok(PlanDraft { id, plan })
}

fn workspace_root(store: &Store, id: i64) -> Result<PathBuf, String> {
    Ok(store.get_workspace(id).map_err(|e| e.to_string())?.root())
}

fn registry<'a>(
    store: &'a Store,
    root: &'a std::path::Path,
    plan_id: u64,
) -> Result<Registry<'a>, String> {
    Ok(Registry {
        store,
        root,
        team: i64::try_from(plan_id).map_err(|e| e.to_string())?,
    })
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
    let root = workspace_root(&store, workspace_id)?;
    folder_trusted(&root)
}

/// Git runner for a workspace root (program resolved by the policy, Claude env stripped).
fn git_for(store: &Store, workspace_id: i64) -> Result<worktree::Git, String> {
    let root = workspace_root(store, workspace_id)?;
    let root = policy::simplify(root.canonicalize().map_err(|e| e.to_string())?);
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let exe = policy::resolve_program(Program::Git, &path_var).map_err(|e| e.to_string())?;
    let env_remove = policy::stripped_env(std::env::vars_os().map(|(k, _)| k));
    Ok(worktree::Git::new(exe, root, env_remove))
}

/// Approved team plus its git runner, with agents in dependency order.
fn team_git(
    store: &Store,
    plans: &PlanStore,
    plan_id: u64,
) -> Result<(worktree::Git, Vec<(String, worktree::Worktree)>), String> {
    let team = plans.approved(store, plan_id).map_err(|e| e.to_string())?;
    let git = git_for(store, team.workspace_id)?;
    let ordered = team
        .plan
        .topo_order()
        .into_iter()
        .filter_map(|a| team.worktrees.get(&a.id).map(|w| (a.id.clone(), w.clone())))
        .collect();
    Ok((git, ordered))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentBranch {
    agent_id: String,
    status: worktree::BranchStatus,
}

#[tauri::command]
async fn team_status(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<Vec<AgentBranch>, String> {
    let (git, team) = team_git(&store, &plans, plan_id)?;
    blocking(move || {
        team.into_iter()
            .map(|(agent_id, wt)| {
                let status = git.status(&wt).map_err(|e| e.to_string())?;
                Ok(AgentBranch { agent_id, status })
            })
            .collect()
    })
    .await
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum IntegrateStep {
    #[serde(rename_all = "camelCase")]
    Merged { agent_id: String },
    #[serde(rename_all = "camelCase")]
    Skipped { agent_id: String },
    #[serde(rename_all = "camelCase")]
    Conflict {
        agent_id: String,
        files: Vec<String>,
    },
    /// Not a conflict (e.g. a hook refused): integration stopped; earlier merges stay.
    #[serde(rename_all = "camelCase")]
    Failed { agent_id: String, error: String },
}

/// Merge every agent branch into the workspace HEAD in dependency order. Refuses if the root
/// or any agent has uncommitted changes; stops (after `merge --abort`) at the first conflict.
#[tauri::command]
async fn team_integrate(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<Vec<IntegrateStep>, String> {
    let (git, team) = team_git(&store, &plans, plan_id)?;
    blocking(move || {
        if !git.root_is_clean().map_err(|e| e.to_string())? {
            return Err("workspace has uncommitted changes; commit or stash them first".into());
        }
        let mut statuses = Vec::new();
        for (id, wt) in &team {
            let s = git.status(wt).map_err(|e| e.to_string())?;
            if s.dirty {
                return Err(format!("agent {id} has uncommitted changes"));
            }
            statuses.push(s);
        }
        let mut steps = Vec::new();
        for ((agent_id, wt), s) in team.into_iter().zip(statuses) {
            if s.ahead == 0 {
                steps.push(IntegrateStep::Skipped { agent_id });
                continue;
            }
            match git.merge(&wt.branch) {
                Ok(worktree::MergeOutcome::Merged) => {
                    steps.push(IntegrateStep::Merged { agent_id })
                }
                Ok(worktree::MergeOutcome::Conflict { files }) => {
                    steps.push(IntegrateStep::Conflict { agent_id, files });
                    break;
                }
                Err(e) => {
                    steps.push(IntegrateStep::Failed {
                        agent_id,
                        error: e.to_string(),
                    });
                    break;
                }
            }
        }
        Ok(steps)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupResult {
    agent_id: String,
    error: Option<String>,
}

/// Stop the team's sessions, then remove agent worktrees and merged branches. Git refuses
/// dirty worktrees and unmerged branches, so no work is lost. When everything is removed the
/// team is closed.
#[tauri::command]
async fn team_cleanup(
    pty: State<'_, PtyManager>,
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
    discard_untracked: bool,
) -> Result<Vec<CleanupResult>, String> {
    let (git, team) = team_git(&store, &plans, plan_id)?;
    let sessions = plans.take_sessions(plan_id);
    for id in &sessions {
        if pty.is_running(id)
            && let Err(e) = pty.kill(id)
        {
            eprintln!("cleanup: kill {id} failed: {e}");
        }
    }
    // A killed process releases its cwd only once it has exited.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while sessions.iter().any(|id| pty.is_running(id)) && std::time::Instant::now() < deadline {
        blocking(|| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            Ok(())
        })
        .await?;
    }
    let results: Vec<CleanupResult> = blocking(move || {
        Ok(team
            .into_iter()
            .map(|(agent_id, wt)| CleanupResult {
                agent_id,
                error: git
                    .cleanup(&wt, discard_untracked)
                    .err()
                    .map(|e| e.to_string()),
            })
            .collect())
    })
    .await?;
    if results.iter().all(|r| r.error.is_none()) {
        plans.close(&store, plan_id).map_err(|e| e.to_string())?;
    }
    Ok(results)
}

/// Approve a plan: create one git worktree + branch per agent (all or nothing), then record it.
#[tauri::command]
async fn plan_approve(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<(), String> {
    let (workspace_id, plan) = plans.pending(&store, plan_id).map_err(|e| e.to_string())?;
    let git = git_for(&store, workspace_id)?;
    // Run id: unique per approval and across restarts, so branch names never collide.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let run = format!("{secs:x}");
    let ids: Vec<String> = plan.agents.iter().map(|a| a.id.clone()).collect();
    let (git, team) = blocking(move || {
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let team = git.create_team(&run, &refs).map_err(|e| e.to_string())?;
        Ok((git, team))
    })
    .await?;
    let team_id = to_team(plan_id)?;
    for (agent, wt) in plan.agents.iter().zip(&team) {
        if let Err(e) = tasks::write_task(&wt.path, team_id, &plan, agent, &wt.branch) {
            git.remove_team(&team);
            return Err(e.to_string());
        }
    }
    let map = plan
        .agents
        .iter()
        .map(|a| a.id.clone())
        .zip(team.iter().cloned())
        .collect();
    plans.approve(&store, plan_id, &map).map_err(|e| {
        git.remove_team(&team);
        e.to_string()
    })?;
    let root = workspace_root(&store, workspace_id)?;
    let agent_ids: Vec<&str> = plan.agents.iter().map(|a| a.id.as_str()).collect();
    registry(&store, &root, plan_id)?
        .register(&agent_ids)
        .map_err(|e| e.to_string())
}

fn to_team(plan_id: u64) -> Result<i64, String> {
    i64::try_from(plan_id).map_err(|e| e.to_string())
}

/// Whether another session of the same agent is still alive (e.g. a remounted terminal).
fn other_live_session(
    pty: &PtyManager,
    plans: &PlanStore,
    team: u64,
    agent: &str,
    not: &str,
) -> bool {
    plans
        .agent_sessions(team, agent)
        .iter()
        .any(|s| s != not && pty.is_running(s))
}

/// Opens an interactive `claude` session for the orchestrator or one agent of an approved plan.
/// The prompt is built here from the stored plan, never taken from the frontend.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn pty_spawn_agent(
    app: tauri::AppHandle,
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
    let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
    let root = workspace_root(&store, team.workspace_id)?;
    if !folder_trusted(&root)? {
        return Err(trust::NOT_TRUSTED.into());
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let reg = registry(&store, &root, plan_id)?;
    let is_agent = agent_id != plan::ORCHESTRATOR_ID;
    // Orchestrator works in the workspace root; each agent in its own worktree.
    let (args, cwd) = if !is_agent {
        let prompt = plan::orchestrator_prompt(&team.plan, &team.worktrees);
        (plan::orchestrator_args(prompt), root.clone())
    } else {
        let agent = team.plan.agent(&agent_id).map_err(|e| e.to_string())?;
        let wt = team
            .worktrees
            .get(&agent_id)
            .ok_or("agent has no worktree")?;
        let waiting: Vec<&str> = team
            .plan
            .prerequisites(&agent_id)
            .into_iter()
            .filter(|p| !matches!(reg.status(p), Ok(Some(TaskStatus::Done))))
            .collect();
        if !waiting.is_empty() {
            return Err(format!("waiting for: {}", waiting.join(", ")));
        }
        // A finished task is being run again: refresh the assignment, set the old result aside.
        if !matches!(
            reg.status(&agent_id).map_err(|e| e.to_string())?,
            None | Some(TaskStatus::Pending | TaskStatus::Running)
        ) {
            tasks::write_task(&wt.path, reg.team, &team.plan, agent, &wt.branch)
                .map_err(|e| e.to_string())?;
        }
        let prompt = plan::agent_prompt(&team.plan, agent, &wt.branch);
        (vec![prompt], wt.path.clone())
    };
    let mut spec = policy::authorize(Program::Claude, args, &root, &cwd, pty.count(), &path_var)
        .map_err(|e| e.to_string())?;
    spec.cols = cols.max(1);
    spec.rows = rows.max(1);
    let sink = channel_sink(on_event);
    let sink: pty::EventSink = if is_agent {
        let agent = agent_id.clone();
        let wt = cwd.clone();
        let root = root.clone();
        Arc::new(move |sid: &str, ev: PtyEvent| {
            if let PtyEvent::Exited { code, killed } = &ev {
                agent_exited(&app, plan_id, &agent, sid, &root, &wt, *code, *killed);
            }
            sink(sid, ev);
        })
    } else {
        sink
    };
    // Registered and Running before the process starts, so an immediate exit is recorded.
    plans.add_session(plan_id, &id, &agent_id);
    if is_agent {
        reg.transition(
            &agent_id,
            TaskStatus::Running,
            None,
            Some("terminal opened"),
        )
        .map_err(|e| e.to_string())?;
    }
    if let Err(e) = pty.spawn(&id, spec, sink) {
        if is_agent && !other_live_session(&pty, &plans, plan_id, &agent_id, &id) {
            let detail = format!("spawn failed: {e}");
            if let Err(e2) = reg.transition(&agent_id, TaskStatus::Failed, None, Some(&detail)) {
                eprintln!("task {agent_id}: recording spawn failure failed: {e2}");
            }
        }
        return Err(e.to_string());
    }
    Ok(())
}

/// An agent's process ended: cancelled if INA killed it, otherwise its result (or the lack of
/// one). Ignored while another terminal of the same agent is still running.
#[allow(clippy::too_many_arguments)]
fn agent_exited(
    app: &tauri::AppHandle,
    plan_id: u64,
    agent: &str,
    session: &str,
    root: &std::path::Path,
    worktree: &std::path::Path,
    code: Option<u32>,
    killed: bool,
) {
    let pty = app.state::<PtyManager>();
    let plans = app.state::<PlanStore>();
    if other_live_session(&pty, &plans, plan_id, agent, session) {
        return;
    }
    let store = app.state::<Store>();
    let result = registry(&store, root, plan_id).and_then(|reg| {
        let done = if killed {
            reg.transition(agent, TaskStatus::Cancelled, None, Some("stopped"))
                .map(|_| ())
        } else {
            let why = match code {
                Some(c) => format!("exited with code {c}"),
                None => "exited".into(),
            };
            reg.finish(agent, worktree, &why)
        };
        done.map_err(|e| e.to_string())
    });
    if let Err(e) = result {
        eprintln!("task {agent}: recording exit failed: {e}");
    }
}

/// One agent's entry in the shared task registry, as shown in the task panel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskView {
    agent_id: String,
    status: TaskStatus,
    summary: Option<String>,
    detail: Option<String>,
    updated_at: String,
    /// `result.json` exists but is invalid; the agent may still fix it.
    result_error: Option<String>,
}

/// Re-read agents' `result.json` and return the team's task registry. A running task whose
/// session is gone (e.g. INA restarted) is finished with whatever result it left.
#[tauri::command]
async fn team_tasks(app: tauri::AppHandle, plan_id: u64) -> Result<Vec<TaskView>, String> {
    blocking(move || {
        let (pty, store, plans) = (
            app.state::<PtyManager>(),
            app.state::<Store>(),
            app.state::<PlanStore>(),
        );
        let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
        let root = workspace_root(&store, team.workspace_id)?;
        let reg = registry(&store, &root, plan_id)?;
        let mut errors = std::collections::HashMap::new();
        for (agent, wt) in &team.worktrees {
            if reg.status(agent).map_err(|e| e.to_string())? != Some(TaskStatus::Running) {
                continue;
            }
            if other_live_session(&pty, &plans, plan_id, agent, "") {
                if let Err(e) = reg.apply_result(agent, &wt.path) {
                    errors.insert(agent.clone(), e.to_string());
                }
            } else {
                reg.finish(agent, &wt.path, "session lost")
                    .map_err(|e| e.to_string())?;
            }
        }
        store
            .tasks(reg.team)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| {
                Ok(TaskView {
                    status: TaskStatus::parse(&t.status).map_err(|e| e.to_string())?,
                    result_error: errors.remove(&t.agent_id),
                    agent_id: t.agent_id,
                    summary: t.summary,
                    detail: t.detail,
                    updated_at: t.updated_at,
                })
            })
            .collect()
    })
    .await
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

/// First-run check of the Claude Code CLI (installed, onboarded, logged in).
#[tauri::command]
async fn claude_setup() -> Result<setup::ClaudeSetup, String> {
    blocking(|| Ok(setup::check())).await
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
            workspace_trusted,
            team_status,
            team_integrate,
            team_cleanup,
            team_list,
            team_tasks,
            claude_setup
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
