mod activity;
mod home;
mod messages;
mod plan;
mod policy;
mod pty;
mod report;
mod setup;
mod store;
mod tasks;
mod trust;
mod usage;
mod verify;
mod worktree;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use plan::{Plan, PlanStore};
use policy::Program;
use pty::{PtyEvent, PtyManager};
use serde::Serialize;
use store::{Store, Workspace};
use tasks::{Registry, TaskStatus};
use tauri::ipc::Channel;
use tauri::{Emitter, Manager, State};

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

/// A plan generated but not approved yet, so a reloaded window can show it again.
#[tauri::command]
fn plan_draft(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    workspace_id: i64,
) -> Result<Option<PlanDraft>, String> {
    Ok(plans
        .latest_draft(&store, workspace_id)
        .map_err(|e| e.to_string())?
        .map(|(id, plan)| PlanDraft { id, plan }))
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

/// Branches of `agent`'s prerequisites (what INA merged into its worktree).
fn prerequisite_branches(team: &plan::Team, agent: &str) -> Vec<String> {
    team.plan
        .prerequisites(agent)
        .into_iter()
        .filter_map(|p| team.worktrees.get(p).map(|w| w.branch.clone()))
        .collect()
}

/// An approved team and its workspace root.
fn approved_team(
    store: &Store,
    plans: &PlanStore,
    plan_id: u64,
) -> Result<(plan::Team, PathBuf), String> {
    let team = plans.approved(store, plan_id).map_err(|e| e.to_string())?;
    let root = workspace_root(store, team.workspace_id)?;
    Ok((team, root))
}

fn workspace_root(store: &Store, id: i64) -> Result<PathBuf, String> {
    Ok(store.get_workspace(id).map_err(|e| e.to_string())?.root())
}

fn registry<'a>(
    store: &'a Store,
    root: &'a std::path::Path,
    plan_id: u64,
    plan: &'a Plan,
) -> Result<Registry<'a>, String> {
    Ok(Registry {
        store,
        root,
        team: i64::try_from(plan_id).map_err(|e| e.to_string())?,
        plan,
    })
}

/// An agent that runs longer than this is stopped and its task failed.
const AGENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);

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

fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "no home directory".into())
}

/// The folder new workspaces go in, created on demand.
fn home_base() -> Result<PathBuf, String> {
    let base = home::base(&home_dir()?);
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(policy::simplify(
        base.canonicalize().map_err(|e| e.to_string())?,
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HomeBase {
    path: String,
    trusted: bool,
}

#[tauri::command]
fn home_info() -> Result<HomeBase, String> {
    let base = home_base()?;
    Ok(HomeBase {
        trusted: folder_trusted(&base)?,
        path: base.display().to_string(),
    })
}

/// Reads what the user wants from the home screen (no tools: it can only answer).
#[tauri::command]
async fn home_intent(request: String) -> Result<home::Intent, String> {
    let request = request.trim().to_string();
    if request.is_empty() || request.len() > home::MAX_REQUEST_LEN {
        return Err("request is empty or too long".into());
    }
    let base = home_base()?;
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let spec = policy::authorize(
        Program::Claude,
        home::intent_args(&request),
        &base,
        &base,
        0,
        &path_var,
    )
    .map_err(|e| e.to_string())?;
    blocking(move || {
        let mut child = pty::piped_command(&spec)
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;
        let (status, output) = pty::wait_output(&mut child, std::time::Duration::from_secs(180))
            .map_err(|e| e.to_string())?;
        status.ok_or("timed out")?;
        home::parse_output(&String::from_utf8_lossy(&output.stdout))
    })
    .await
}

/// New workspace `<base>/<name>` as a git repo with one commit, added to INA.
#[tauri::command]
async fn workspace_create(store: State<'_, Store>, name: String) -> Result<Workspace, String> {
    let base = home_base()?;
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let exe = policy::resolve_program(Program::Git, &path_var).map_err(|e| e.to_string())?;
    let env_remove = policy::stripped_env(std::env::vars_os().map(|(k, _)| k));
    let dir = blocking(move || {
        let dir = home::create_dir(&base, &name)?;
        worktree::Git::new(exe, dir.clone(), env_remove)
            .init_repo()
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(dir)
    })
    .await?;
    store.add_workspace(&dir).map_err(|e| e.to_string())
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
/// A team's git runner, its worktrees in dependency order, and the team itself.
type TeamGit = (worktree::Git, Vec<(String, worktree::Worktree)>, plan::Team);

fn team_git(store: &Store, plans: &PlanStore, plan_id: u64) -> Result<TeamGit, String> {
    let team = plans.approved(store, plan_id).map_err(|e| e.to_string())?;
    let git = git_for(store, team.workspace_id)?;
    let ordered = team
        .plan
        .topo_order()
        .into_iter()
        .filter_map(|a| team.worktrees.get(&a.id).map(|w| (a.id.clone(), w.clone())))
        .collect();
    Ok((git, ordered, team))
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
    let (git, team, _) = team_git(&store, &plans, plan_id)?;
    blocking(move || {
        // Independent read-only git calls, one worktree each: run them side by side.
        std::thread::scope(|s| {
            let handles: Vec<_> = team
                .into_iter()
                .map(|(agent_id, wt)| {
                    let git = &git;
                    s.spawn(move || {
                        let status = git.status(&wt).map_err(|e| e.to_string())?;
                        Ok(AgentBranch { agent_id, status })
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().map_err(|_| "git status panicked".to_string())?)
                .collect()
        })
    })
    .await
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum PreviewStep {
    #[serde(rename_all = "camelCase")]
    Clean { agent_id: String },
    #[serde(rename_all = "camelCase")]
    Skipped { agent_id: String },
    #[serde(rename_all = "camelCase")]
    Conflict {
        agent_id: String,
        files: Vec<String>,
    },
    /// After a predicted conflict integration would stop, so this branch is not attempted.
    #[serde(rename_all = "camelCase")]
    NotReached { agent_id: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overlap {
    file: String,
    agents: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrationPreview {
    steps: Vec<PreviewStep>,
    /// Files changed by more than one agent (possible semantic conflicts even if they merge).
    overlaps: Vec<Overlap>,
}

/// What `team_integrate` would do, computed without changing anything.
#[tauri::command]
async fn team_preview(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<IntegrationPreview, String> {
    let (git, team, approved) = team_git(&store, &plans, plan_id)?;
    blocking(move || {
        let mut ahead = Vec::new();
        let mut by_file: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (id, wt) in &team {
            let s = git.status(wt).map_err(|e| e.to_string())?;
            let prereq_branches = prerequisite_branches(&approved, id);
            let own = git
                .own_files(&wt.branch, &prereq_branches)
                .map_err(|e| e.to_string())?;
            for f in own {
                by_file.entry(f).or_default().push(id.clone());
            }
            if s.ahead > 0 {
                ahead.push((id.clone(), wt.branch.clone()));
            }
        }
        let branches: Vec<String> = ahead.iter().map(|(_, b)| b.clone()).collect();
        let results = git.preview_merges(&branches).map_err(|e| e.to_string())?;
        let mut steps = Vec::new();
        for (id, _) in &team {
            let Some(i) = ahead.iter().position(|(a, _)| a == id) else {
                steps.push(PreviewStep::Skipped {
                    agent_id: id.clone(),
                });
                continue;
            };
            steps.push(match results.get(i) {
                Some(None) => PreviewStep::Clean {
                    agent_id: id.clone(),
                },
                Some(Some(files)) => PreviewStep::Conflict {
                    agent_id: id.clone(),
                    files: files.clone(),
                },
                None => PreviewStep::NotReached {
                    agent_id: id.clone(),
                },
            });
        }
        let overlaps = by_file
            .into_iter()
            .filter(|(_, a)| a.len() > 1)
            .map(|(file, agents)| Overlap { file, agents })
            .collect();
        Ok(IntegrationPreview { steps, overlaps })
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
    let (git, team, approved) = team_git(&store, &plans, plan_id)?;
    // Every open team has registry rows (startup backfills teams approved before it existed).
    let root = workspace_root(&store, approved.workspace_id)?;
    let task_statuses: HashMap<String, TaskStatus> =
        registry(&store, &root, plan_id, &approved.plan)?
            .tasks()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|t| (t.agent_id, t.status))
            .collect();
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
            let done = task_statuses
                .get(id)
                .is_some_and(|st| *st == TaskStatus::Done);
            if s.ahead > 0 && !done {
                return Err(format!("agent {id} is not done (verified) yet"));
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
/// Hides a team from INA: stops its sessions and closes it. Worktrees and branches stay on disk
/// (nothing is deleted), so its work can still be merged or inspected with git.
#[tauri::command]
fn team_archive(
    pty: State<'_, PtyManager>,
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<(), String> {
    for id in plans.take_sessions(plan_id) {
        if pty.is_running(&id)
            && let Err(e) = pty.kill(&id)
        {
            eprintln!("archive: kill {id} failed: {e}");
        }
    }
    plans.close(&store, plan_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn team_cleanup(
    pty: State<'_, PtyManager>,
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
    discard_untracked: bool,
) -> Result<Vec<CleanupResult>, String> {
    let (git, team, _) = team_git(&store, &plans, plan_id)?;
    let _cleaning = plans.cleaning(plan_id);
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
    clis: HashMap<String, plan::AgentCli>,
) -> Result<(), String> {
    plans
        .set_clis(&store, plan_id, &clis)
        .map_err(|e| e.to_string())?;
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
        if let Err(e) = tasks::write_task(&wt.path, team_id, &plan, agent, &wt.branch, &[], None) {
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
    registry(&store, &root, plan_id, &plan)?
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

fn to_db_id(plan_id: u64) -> Result<i64, String> {
    i64::try_from(plan_id).map_err(|e| e.to_string())
}

/// The agent's last Claude session, if its log still exists (Claude agents and orchestrator).
fn resumable_session(
    store: &Store,
    team: &plan::Team,
    plan_id: u64,
    agent_id: &str,
) -> Result<Option<String>, String> {
    let is_claude = agent_id == plan::ORCHESTRATOR_ID
        || team
            .plan
            .agent(agent_id)
            .is_ok_and(|a| a.cli == plan::AgentCli::Claude);
    if !is_claude {
        return Ok(None);
    }
    let cwd = if agent_id == plan::ORCHESTRATOR_ID {
        workspace_root(store, team.workspace_id)?
    } else {
        match team.worktrees.get(agent_id) {
            Some(wt) => wt.path.clone(),
            None => return Ok(None),
        }
    };
    let sessions = store
        .agent_sessions_of(to_db_id(plan_id)?)
        .map_err(|e| e.to_string())?;
    Ok(sessions
        .into_iter()
        .find(|(a, _)| a == agent_id)
        .map(|(_, sid)| sid)
        .filter(|sid| activity::find_log(plan::AgentCli::Claude, &cwd, Some(sid)).is_some()))
}

/// Members of a team whose last conversation can be continued.
#[tauri::command]
fn team_resumable(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<Vec<String>, String> {
    let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
    let mut ids = vec![plan::ORCHESTRATOR_ID.to_string()];
    ids.extend(team.plan.agents.iter().map(|a| a.id.clone()));
    let mut out = vec![];
    for id in ids {
        if resumable_session(&store, &team, plan_id, &id)?.is_some() {
            out.push(id);
        }
    }
    Ok(out)
}

/// Opens an interactive `claude` session for the orchestrator or one agent of an approved plan.
/// The prompt is built here from the stored plan, never taken from the frontend.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn pty_spawn_agent(
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
    resume: Option<bool>,
) -> Result<(), String> {
    valid_id(&id)?;
    let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
    // Continue the agent's last Claude conversation instead of sending its task again.
    let resume_id = if resume == Some(true) {
        Some(
            resumable_session(&store, &team, plan_id, &agent_id)?
                .ok_or("no conversation to resume")?,
        )
    } else {
        None
    };
    let root = workspace_root(&store, team.workspace_id)?;
    if !folder_trusted(&root)? {
        return Err(trust::NOT_TRUSTED.into());
    }
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let reg = registry(&store, &root, plan_id, &team.plan)?;
    let is_agent = agent_id != plan::ORCHESTRATOR_ID;
    // Orchestrator works in the workspace root; each agent in its own worktree.
    let (program, args, cwd) = if !is_agent {
        let prompt = plan::orchestrator_prompt(&team.plan, &team.worktrees);
        (
            Program::Claude,
            plan::orchestrator_args(prompt),
            root.clone(),
        )
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
        // A finished task is being run again: set the old result aside and tell the agent how
        // the previous run ended.
        let row = reg.task(&agent_id).map_err(|e| e.to_string())?;
        let previous = match row.filter(|_| resume_id.is_none()) {
            Some(t) => {
                let status = t.status;
                let finished = !matches!(
                    status,
                    TaskStatus::Pending | TaskStatus::Running | TaskStatus::Verifying
                );
                finished.then_some(tasks::PreviousAttempt {
                    status,
                    summary: t.summary,
                    detail: t.detail,
                })
            }
            None => None,
        };
        let previous = if previous.is_some() {
            tasks::archive_result(&wt.path).map_err(|e| e.to_string())?;
            previous
        } else {
            // Same run reopened (e.g. a remounted terminal): keep what the first spawn recorded.
            tasks::recorded_previous(&wt.path)
        };
        // Never merge under a live session of this agent: it may be editing that worktree.
        let merge = !other_live_session(&pty, &plans, plan_id, &agent_id, "");
        let prerequisites = merge_prerequisites(&store, &team, &agent_id, wt, merge).await?;
        tasks::write_task(
            &wt.path,
            reg.team,
            &team.plan,
            agent,
            &wt.branch,
            &prerequisites,
            previous.as_ref(),
        )
        .map_err(|e| e.to_string())?;
        let prompt = plan::agent_prompt(&team.plan, agent, &wt.branch);
        (agent.cli.program(), vec![prompt], wt.path.clone())
    };
    let mut args = args;
    if program == Program::Claude {
        // Status line that records plan usage for the footer (usage.rs).
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        if let Some(s) = usage::claude_settings(&exe, &claude_usage_file(&app)?) {
            args.extend(["--settings".to_string(), s]);
        }
        // Known session id: the interface reads exactly this session's log. Resuming keeps the
        // same id (and log) and drops the prompt: the task is already in that conversation.
        let session_id = match &resume_id {
            Some(sid) => {
                args.remove(0);
                args.extend(["--resume".to_string(), sid.clone()]);
                sid.clone()
            }
            None => {
                let sid = activity::new_session_id();
                args.extend(["--session-id".to_string(), sid.clone()]);
                sid
            }
        };
        plans.set_log(plan_id, &agent_id, &session_id);
        if let Err(e) = store.set_agent_session(to_db_id(plan_id)?, &agent_id, &session_id) {
            eprintln!("session {agent_id}: not recorded: {e}");
        }
    }
    let mut spec = policy::authorize(program, args, &root, &cwd, pty.count(), &path_var)
        .map_err(|e| e.to_string())?;
    spec.cols = cols.max(1);
    spec.rows = rows.max(1);
    let sink = channel_sink(on_event);
    let app_for_timeout = app.clone();
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
    let spawned = pty.spawn(&id, spec, sink);
    if spawned.is_ok() && is_agent {
        app_for_timeout
            .state::<Deadlines>()
            .watch(id.clone(), plan_id, agent_id.clone());
    }
    if let Err(e) = spawned {
        if is_agent && !other_live_session(&pty, &plans, plan_id, &agent_id, &id) {
            // Stopped before it started (e.g. the terminal unmounted): a cancel, not a failure.
            let (to, detail) = match e {
                pty::PtyError::StoppedBeforeStart(_) => (TaskStatus::Cancelled, "stopped".into()),
                _ => (TaskStatus::Failed, format!("spawn failed: {e}")),
            };
            if let Err(e2) = reg.transition(&agent_id, to, None, Some(&detail)) {
                eprintln!("task {agent_id}: recording spawn failure failed: {e2}");
            }
        }
        return Err(e.to_string());
    }
    Ok(())
}

/// One lock per worktree path, for git operations that must not interleave in it.
fn worktree_lock(dir: &std::path::Path) -> Result<Arc<Mutex<()>>, String> {
    static LOCKS: Mutex<Option<HashMap<PathBuf, Arc<Mutex<()>>>>> = Mutex::new(None);
    let mut locks = LOCKS.lock().map_err(|e| e.to_string())?;
    let locks = locks.get_or_insert_default();
    // Drop locks nobody holds or waits on (only the map references them).
    locks.retain(|_, l| Arc::strong_count(l) > 1);
    Ok(locks.entry(dir.to_path_buf()).or_default().clone())
}

/// Merge every finished prerequisite's branch into the dependent's worktree (so it builds on
/// their work) and describe them for its `task.json`. A conflict is aborted and reported.
async fn merge_prerequisites(
    store: &Store,
    team: &plan::Team,
    agent_id: &str,
    wt: &worktree::Worktree,
    merge: bool,
) -> Result<Vec<tasks::Prerequisite>, String> {
    let rows = store.tasks(to_team(team.id)?).map_err(|e| e.to_string())?;
    let prerequisites: Vec<tasks::Prerequisite> = team
        .plan
        .prerequisites(agent_id)
        .into_iter()
        .filter_map(|p| {
            let branch = team.worktrees.get(p)?.branch.clone();
            let summary = rows.iter().find(|r| r.agent_id == p)?.summary.clone();
            Some(tasks::Prerequisite {
                id: p.to_owned(),
                branch,
                summary,
            })
        })
        .collect();
    if prerequisites.is_empty() || !merge {
        return Ok(prerequisites);
    }
    let git = git_for(store, team.workspace_id)?;
    let dir = wt.path.clone();
    let branches: Vec<String> = prerequisites.iter().map(|p| p.branch.clone()).collect();
    blocking(move || {
        // Two spawns of one agent (e.g. a remounted terminal) must not race on its git index;
        // other worktrees merge independently.
        let lock = worktree_lock(&dir)?;
        let _guard = lock.lock().map_err(|e| e.to_string())?;
        for b in &branches {
            if git.is_merged_in(&dir, b) {
                continue;
            }
            match git.merge_in(&dir, b).map_err(|e| e.to_string())? {
                worktree::MergeOutcome::Merged => {}
                worktree::MergeOutcome::Conflict { files } => {
                    return Err(format!(
                        "conflict merging prerequisite {b} into this worktree: {}",
                        files.join(", ")
                    ));
                }
            }
        }
        Ok(())
    })
    .await?;
    Ok(prerequisites)
}

/// Agent sessions and when they time out; checked by the background tick.
#[derive(Default)]
struct Deadlines(Mutex<HashMap<String, (u64, String, std::time::Instant)>>);

impl Deadlines {
    fn watch(&self, session: String, plan_id: u64, agent: String) {
        let deadline = std::time::Instant::now() + AGENT_TIMEOUT;
        match self.0.lock() {
            Ok(mut d) => {
                d.insert(session, (plan_id, agent, deadline));
            }
            Err(e) => eprintln!("task {agent}: timeout not watched: {e}"),
        }
    }

    /// Forget ended sessions; return the live ones past their deadline.
    fn expired(&self, pty: &PtyManager) -> Vec<(String, u64, String)> {
        let Ok(mut d) = self.0.lock() else {
            return Vec::new();
        };
        let now = std::time::Instant::now();
        d.retain(|session, _| pty.is_running(session));
        let late: Vec<String> = d
            .iter()
            .filter(|(_, (_, _, at))| *at <= now)
            .map(|(s, _)| s.clone())
            .collect();
        late.into_iter()
            .filter_map(|s| d.remove(&s).map(|(plan, agent, _)| (s, plan, agent)))
            .collect()
    }
}

/// Fail and stop agent sessions that outlive `AGENT_TIMEOUT`.
fn reap_timeouts(app: &tauri::AppHandle) {
    let pty = app.state::<PtyManager>();
    for (session, plan_id, agent) in app.state::<Deadlines>().expired(&pty) {
        let (store, plans) = (app.state::<Store>(), app.state::<PlanStore>());
        let failed = plans
            .approved(&store, plan_id)
            .map_err(|e| e.to_string())
            .and_then(|team| {
                let root = workspace_root(&store, team.workspace_id)?;
                let why = format!("timed out after {} min", AGENT_TIMEOUT.as_secs() / 60);
                let reg = registry(&store, &root, plan_id, &team.plan)?;
                // Already reported (e.g. verifying): the result is in, nothing to time out.
                if reg.status(&agent).map_err(|e| e.to_string())? != Some(TaskStatus::Running) {
                    return Ok(false);
                }
                reg.transition(&agent, TaskStatus::Failed, None, Some(&why))
                    .map_err(|e| e.to_string())
            });
        match failed {
            // Failed first, so the kill's exit is not recorded as a cancel.
            Ok(true) => {
                if let Err(e) = pty.kill(&session) {
                    eprintln!("task {agent}: timeout kill failed: {e}");
                }
            }
            // Reported meanwhile (verifying, blocked, done): leave the session alone.
            Ok(false) => {}
            Err(e) => eprintln!("task {agent}: recording timeout failed: {e}"),
        }
    }
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
    let team = match plans.approved(&store, plan_id) {
        Ok(t) => t,
        Err(e) => return eprintln!("task {agent}: recording exit failed: {e}"),
    };
    let result = registry(&store, root, plan_id, &team.plan).and_then(|reg| {
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
    let kicked = result.and_then(|_| {
        let rows = registry(&store, root, plan_id, &team.plan)?
            .tasks()
            .map_err(|e| e.to_string())?;
        kick_verifications(app, plan_id, &team, &rows)
    });
    if let Err(e) = kicked {
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
    /// Agents that must be done before this one may start (`Plan::prerequisites`).
    prerequisites: Vec<String>,
}

/// Start verification for every `verifying` task of a team that is not being verified yet.
/// Called from the poll, on exit and after a restart, so a lost run is simply restarted.
fn kick_verifications(
    app: &tauri::AppHandle,
    plan_id: u64,
    team: &plan::Team,
    rows: &[tasks::Task],
) -> Result<(), String> {
    static RUNNING: Mutex<Option<std::collections::HashSet<(u64, String)>>> = Mutex::new(None);
    /// Releases the claim when the verification thread ends, even if it panics.
    struct Claim((u64, String));
    impl Drop for Claim {
        fn drop(&mut self) {
            if let Ok(mut running) = RUNNING.lock() {
                running.get_or_insert_default().remove(&self.0);
            }
        }
    }
    for t in rows {
        if t.status != TaskStatus::Verifying {
            continue;
        }
        let key = (plan_id, t.agent_id.clone());
        {
            let mut running = RUNNING.lock().map_err(|e| e.to_string())?;
            if !running.get_or_insert_default().insert(key.clone()) {
                continue;
            }
        }
        let claim = Claim(key);
        let app = app.clone();
        let team = team.clone();
        std::thread::spawn(move || {
            let key = &claim.0;
            if let Err(e) = verify_agent(&app, plan_id, &team, &key.1) {
                eprintln!("verify {}: {e}", key.1);
                // Not left in `verifying`, where every poll would retry it.
                let store = app.state::<Store>();
                let failed = workspace_root(&store, team.workspace_id).and_then(|root| {
                    registry(&store, &root, plan_id, &team.plan)?
                        .transition(
                            &key.1,
                            TaskStatus::Failed,
                            None,
                            Some(&format!("verification error: {e}")),
                        )
                        .map_err(|e| e.to_string())
                });
                if let Err(e2) = failed {
                    eprintln!("verify {}: recording failure failed: {e2}", key.1);
                }
            }
        });
    }
    Ok(())
}

/// Run one agent's checks, compare declared files with its branch, then mark it done
/// (verified, or explicitly unverified when it has no checks) or failed.
fn verify_agent(
    app: &tauri::AppHandle,
    plan_id: u64,
    team: &plan::Team,
    agent_id: &str,
) -> Result<(), String> {
    let store = app.state::<Store>();
    let agent = team.plan.agent(agent_id).map_err(|e| e.to_string())?;
    let wt = team
        .worktrees
        .get(agent_id)
        .ok_or("agent has no worktree")?;
    let root = workspace_root(&store, team.workspace_id)?;
    let declared = tasks::read_result(&wt.path)
        .ok()
        .flatten()
        .map(|r| r.files)
        .unwrap_or_default();
    let git = git_for(&store, team.workspace_id)?;
    let prereqs = prerequisite_branches(team, agent_id);
    let changed = git
        .own_files(&wt.branch, &prereqs)
        .map_err(|e| e.to_string())?;
    let mismatch = verify::files_mismatch(&declared, &changed);
    let reg = registry(&store, &root, plan_id, &team.plan)?;
    // Checks see exactly what was committed (what integration would merge), not loose files.
    let snap = root
        .join(tasks::INA_DIR)
        .join("verify")
        .join(format!("{}-{agent_id}", reg.team));
    if snap.exists() {
        // Left over from an interrupted run.
        if let Err(e) = git.remove_snapshot(&snap) {
            eprintln!("verify {agent_id}: removing stale snapshot failed: {e}");
        }
    }
    let outcome = match git.snapshot(&wt.branch, &snap) {
        Ok(()) => {
            let o = verify::run_checks(&agent.verify, &root, &snap);
            if let Err(e) = git.remove_snapshot(&snap) {
                eprintln!("verify {agent_id}: removing snapshot failed: {e}");
            }
            o
        }
        Err(e) => verify::Outcome::Failed {
            check: "checkout".into(),
            detail: e.to_string(),
        },
    };
    let (to, mut detail) = match outcome {
        verify::Outcome::Passed(0) => (TaskStatus::Done, "unverified: no checks".to_string()),
        verify::Outcome::Passed(n) => (TaskStatus::Done, format!("verified: {n} check(s) passed")),
        verify::Outcome::Failed { check, detail } => (
            TaskStatus::Failed,
            format!("verification failed: `{check}` {detail}"),
        ),
    };
    if let Some(m) = mismatch {
        detail.push_str(&format!("; files: {m}"));
    }
    let summary = reg
        .task(agent_id)
        .map_err(|e| e.to_string())?
        .and_then(|t| t.summary);
    reg.transition(agent_id, to, summary.as_deref(), Some(&detail))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// A message as shown in the team panel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageView {
    id: i64,
    from: String,
    to: Option<String>,
    kind: String,
    text: String,
    status: String,
    reply_to: Option<i64>,
    at: String,
}

const MSG_RECEIVED: &str = "received";
const MSG_PROPOSED: &str = "proposed";
const MSG_DELIVERED: &str = "delivered";
const MSG_REJECTED: &str = "rejected";
/// Stored but its inbox file could not be written.
const MSG_UNDELIVERED: &str = "undelivered";

/// Ingest new outbox files (orchestrator in the root, agents in their worktrees) and return
/// the team's messages. Orchestrator instructions wait as `proposed` for the user.
#[tauri::command]
async fn team_messages(app: tauri::AppHandle, plan_id: u64) -> Result<Vec<MessageView>, String> {
    blocking(move || {
        let (team, _) = sync_messages(&app, plan_id)?;
        list_messages(&app.state::<Store>(), team)
    })
    .await
}

/// Ingest every outbox of the team (orchestrator root + agent worktrees) into the store;
/// returns the registry's team id and how many messages arrived.
fn sync_messages(app: &tauri::AppHandle, plan_id: u64) -> Result<(i64, usize), String> {
    let mut arrived = 0;
    let (store, plans) = (app.state::<Store>(), app.state::<PlanStore>());
    let (team, root) = approved_team(&store, &plans, plan_id)?;
    let reg = registry(&store, &root, plan_id, &team.plan)?;
    let agents: Vec<&str> = team.plan.agents.iter().map(|a| a.id.as_str()).collect();
    let mut sources: Vec<(&str, &std::path::Path)> = vec![(plan::ORCHESTRATOR_ID, &root)];
    sources.extend(
        team.worktrees
            .iter()
            .map(|(a, w)| (a.as_str(), w.path.as_path())),
    );
    for (from, dir) in sources {
        let is_orch = from == plan::ORCHESTRATOR_ID;
        arrived += messages::ingest(dir, is_orch, &agents, |got| {
            let (kind, text, to, status) = match &got.message {
                Ok(m) => (
                    m.kind.as_str(),
                    m.text.clone(),
                    m.to.as_deref(),
                    if is_orch { MSG_PROPOSED } else { MSG_RECEIVED },
                ),
                Err(e) => ("invalid", format!("{}: {e}", got.file), None, MSG_REJECTED),
            };
            store
                .insert_message(
                    reg.team,
                    &store::NewMessage {
                        from,
                        to,
                        kind,
                        text: &text,
                        status,
                        reply_to: None,
                    },
                )
                .map_err(|e| e.to_string())?;
            reg.log_event(from, "message", Some(&format!("{kind} ({status})")));
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .len();
    }
    Ok((reg.team, arrived))
}

fn list_messages(store: &Store, team: i64) -> Result<Vec<MessageView>, String> {
    Ok(store
        .messages(team)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|m| MessageView {
            id: m.id,
            from: m.from_agent,
            to: m.to_agent,
            kind: m.kind,
            text: m.text,
            status: m.status,
            reply_to: m.reply_to,
            at: m.at,
        })
        .collect())
}

/// Write a message to an agent's inbox. Its terminal is not touched: typing into a session
/// that shows a permission prompt could answer it, so the user triggers the notice.
fn deliver(
    team: &plan::Team,
    id: i64,
    from: &str,
    to: &str,
    kind: messages::Kind,
    text: &str,
    reply_to: Option<i64>,
) -> Result<(), String> {
    let wt = team.worktrees.get(to).ok_or("unknown agent")?;
    messages::write_inbox(&wt.path, id, from, kind, text, reply_to)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Tell an agent's live terminal(s) to read its inbox (one fixed line, user-triggered).
/// Returns how many sessions were notified.
#[tauri::command]
fn message_notify(
    pty: State<'_, PtyManager>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
    agent_id: String,
) -> Result<usize, String> {
    let mut n = 0;
    for s in plans.agent_sessions(plan_id, &agent_id) {
        if pty.is_running(&s) {
            pty.write(&s, messages::NOTICE.as_bytes())
                .map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    Ok(n)
}

/// The user answers or instructs an agent directly.
#[tauri::command]
async fn team_send(
    app: tauri::AppHandle,
    plan_id: u64,
    to: String,
    text: String,
    reply_to: Option<i64>,
) -> Result<Vec<MessageView>, String> {
    blocking(move || {
        let (store, plans) = (app.state::<Store>(), app.state::<PlanStore>());
        let text = text.trim();
        if text.is_empty() || text.chars().count() > messages::MAX_TEXT {
            return Err("message is empty or too long".into());
        }
        let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
        team.plan.agent(&to).map_err(|e| e.to_string())?;
        let team_id = to_team(plan_id)?;
        let id = store
            .insert_message(
                team_id,
                &store::NewMessage {
                    from: "user",
                    to: Some(&to),
                    kind: messages::Kind::Reply.as_str(),
                    text,
                    status: MSG_UNDELIVERED,
                    reply_to,
                },
            )
            .map_err(|e| e.to_string())?;
        // Marked delivered only once the inbox file exists.
        let reply = messages::Kind::Reply;
        deliver(&team, id, "user", &to, reply, text, reply_to)?;
        store
            .update_message_status(team_id, id, MSG_UNDELIVERED, MSG_DELIVERED)
            .map_err(|e| e.to_string())?;
        list_messages(&store, team_id)
    })
    .await
}

/// Approve (deliver) or reject an orchestrator instruction.
#[tauri::command]
async fn message_decide(
    app: tauri::AppHandle,
    plan_id: u64,
    message_id: i64,
    approve: bool,
) -> Result<Vec<MessageView>, String> {
    blocking(move || {
        let (store, plans) = (app.state::<Store>(), app.state::<PlanStore>());
        let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
        let team_id = to_team(plan_id)?;
        let msg = store
            .messages(team_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|m| m.id == message_id && m.status == MSG_PROPOSED)
            .ok_or("no such pending proposal")?;
        // Deliver first: a failed delivery leaves the proposal pending, so it can be retried.
        if approve {
            let target = msg.to_agent.as_deref().ok_or("instruction has no target")?;
            let kind = messages::Kind::Instruction;
            deliver(
                &team,
                msg.id,
                &msg.from_agent,
                target,
                kind,
                &msg.text,
                None,
            )?;
        }
        let to = if approve { MSG_DELIVERED } else { MSG_REJECTED };
        if !store
            .update_message_status(team_id, message_id, MSG_PROPOSED, to)
            .map_err(|e| e.to_string())?
        {
            return Err("proposal was already decided".into());
        }
        list_messages(&store, team_id)
    })
    .await
}

/// Re-read running agents' `result.json` and return the team's task registry.
/// Plain final report of a finished team (what was built, how to try it), from the plan and
/// the agents' result summaries. Tool-less `claude -p` in the workspace root.
#[tauri::command]
async fn team_report(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
) -> Result<report::Report, String> {
    let team = plans.approved(&store, plan_id).map_err(|e| e.to_string())?;
    let root = workspace_root(&store, team.workspace_id)?;
    if !folder_trusted(&root)? {
        return Err(trust::NOT_TRUSTED.into());
    }
    let tasks = team_tasks(app, plan_id).await?;
    let parts: Vec<report::Part> = team
        .plan
        .agents
        .iter()
        .map(|a| report::Part {
            name: &a.name,
            task: &a.task,
            summary: tasks
                .iter()
                .find(|t| t.agent_id == a.id)
                .and_then(|t| t.summary.as_deref()),
        })
        .collect();
    let prompt = report::prompt(&team.plan.summary, &parts);
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let spec = policy::authorize(
        Program::Claude,
        report::args(&prompt),
        &root,
        &root,
        0,
        &path_var,
    )
    .map_err(|e| e.to_string())?;
    blocking(move || {
        let mut child = pty::piped_command(&spec)
            .spawn()
            .map_err(|e| format!("spawn: {e}"))?;
        let (status, output) = pty::wait_output(&mut child, std::time::Duration::from_secs(180))
            .map_err(|e| e.to_string())?;
        status.ok_or("timed out")?;
        report::parse_output(&String::from_utf8_lossy(&output.stdout))
    })
    .await
}

/// Where a static site's `index.html` usually lives in a project.
const PREVIEW_PAGES: &[&str] = &[
    "index.html",
    "public/index.html",
    "dist/index.html",
    "build/index.html",
    "site/index.html",
    "docs/index.html",
];

/// The workspace's site entry page, if any, made readable by the preview frame (asset
/// protocol, this workspace only). The frame is sandboxed in the UI.
#[tauri::command]
fn preview_page(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    workspace_id: i64,
) -> Result<Option<String>, String> {
    let root = workspace_root(&store, workspace_id)?;
    let Some(page) = PREVIEW_PAGES
        .iter()
        .map(|p| root.join(p))
        .find(|p| p.is_file())
    else {
        return Ok(None);
    };
    let dir = page.parent().unwrap_or(&root).to_path_buf();
    app.asset_protocol_scope()
        .allow_directory(&dir, true)
        .map_err(|e| e.to_string())?;
    Ok(Some(page.display().to_string()))
}

#[tauri::command]
async fn team_tasks(app: tauri::AppHandle, plan_id: u64) -> Result<Vec<TaskView>, String> {
    blocking(move || sync_tasks(&app, plan_id)).await
}

/// Apply agents' results, start pending verifications and return the registry.
/// Runs from the UI poll and from the background tick (idempotent).
fn sync_tasks(app: &tauri::AppHandle, plan_id: u64) -> Result<Vec<TaskView>, String> {
    let (store, plans) = (app.state::<Store>(), app.state::<PlanStore>());
    let (team, root) = approved_team(&store, &plans, plan_id)?;
    let reg = registry(&store, &root, plan_id, &team.plan)?;
    let mut errors = std::collections::HashMap::new();
    for (agent, wt) in &team.worktrees {
        if let Err(e) = reg.apply_result(agent, &wt.path) {
            errors.insert(agent.clone(), e.to_string());
        }
    }
    let rows = reg.tasks().map_err(|e| e.to_string())?;
    kick_verifications(app, plan_id, &team, &rows)?;
    rows.into_iter()
        .map(|t| {
            Ok(TaskView {
                status: t.status,
                result_error: errors.remove(&t.agent_id),
                prerequisites: team
                    .plan
                    .prerequisites(&t.agent_id)
                    .into_iter()
                    .map(String::from)
                    .collect(),
                agent_id: t.agent_id,
                summary: t.summary,
                detail: t.detail,
                updated_at: t.updated_at,
            })
        })
        .collect()
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

/// Working directory and CLI of a team member (the orchestrator runs Claude in the root).
fn member(
    store: &Store,
    plans: &PlanStore,
    plan_id: u64,
    agent_id: &str,
) -> Result<(PathBuf, plan::AgentCli), String> {
    let team = plans.approved(store, plan_id).map_err(|e| e.to_string())?;
    if agent_id == plan::ORCHESTRATOR_ID {
        return Ok((
            workspace_root(store, team.workspace_id)?,
            plan::AgentCli::Claude,
        ));
    }
    let agent = team.plan.agent(agent_id).map_err(|e| e.to_string())?;
    let wt = team
        .worktrees
        .get(agent_id)
        .ok_or("agent has no worktree")?;
    Ok((wt.path.clone(), agent.cli))
}

/// What a team member did, from its CLI's session log; empty before its first session.
#[tauri::command]
async fn agent_activity(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
    agent_id: String,
) -> Result<Vec<activity::Event>, String> {
    let (cwd, cli) = member(&store, &plans, plan_id, &agent_id)?;
    let session_id = plans.log(plan_id, &agent_id);
    blocking(
        move || match activity::find_log(cli, &cwd, session_id.as_deref()) {
            Some(file) => activity::read_log(cli, &file).map_err(|e| e.to_string()),
            None => Ok(vec![]),
        },
    )
    .await
}

/// Type a line into a team member's live terminal (the interface's message box).
#[tauri::command]
fn agent_send(
    pty: State<'_, PtyManager>,
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    plan_id: u64,
    agent_id: String,
    text: String,
) -> Result<(), String> {
    member(&store, &plans, plan_id, &agent_id)?;
    let line = activity::terminal_input(&text).ok_or("message is empty or too long")?;
    let session = plans
        .agent_sessions(plan_id, &agent_id)
        .into_iter()
        .rev()
        .find(|s| pty.is_running(s))
        .ok_or("no running session for this agent")?;
    pty.write(&session, line.as_bytes())
        .map_err(|e| e.to_string())?;
    // Enter on its own: the TUIs treat text and Enter arriving together as a paste.
    std::thread::sleep(std::time::Duration::from_millis(80));
    pty.write(&session, b"\r").map_err(|e| e.to_string())
}
pub const USAGE_SINK_FLAG: &str = usage::SINK_FLAG;

pub fn usage_sink(file: &std::path::Path) {
    usage::run_sink(file)
}

fn claude_usage_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join(usage::CLAUDE_FILE))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UsageReport {
    /// Last limits a Claude session started by INA reported; `None` before the first one.
    claude: Option<usage::Usage>,
    codex: Result<usage::Usage, String>,
}

/// Plan usage (session and weekly windows) for Claude Code and Codex.
#[tauri::command]
async fn usage_read(app: tauri::AppHandle) -> Result<UsageReport, String> {
    let file = claude_usage_file(&app)?;
    blocking(move || {
        let path_var = std::env::var_os("PATH").unwrap_or_default();
        let codex = policy::resolve_program(Program::Codex, &path_var)
            .map_err(|e| e.to_string())
            .and_then(|exe| usage::read_codex(&exe));
        Ok(UsageReport {
            claude: usage::read_claude(&file),
            codex,
        })
    })
    .await
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
fn workspace_remove(
    store: State<'_, Store>,
    plans: State<'_, PlanStore>,
    id: i64,
) -> Result<(), String> {
    store.remove_workspace(id).map_err(|e| e.to_string())?;
    plans.forget_workspace(id);
    Ok(())
}

/// Finish tasks still `running` with no live session (e.g. two sessions of an agent exited
/// together and each saw the other alive). Only on the second tick in a row, so a spawn
/// between its `Running` transition and its PTY starting is not taken for lost.
fn finish_orphans(
    app: &tauri::AppHandle,
    team: &plan::Team,
    root: &std::path::Path,
    suspects: &mut std::collections::HashSet<(u64, String)>,
) -> Result<(), String> {
    let (store, plans, pty) = (
        app.state::<Store>(),
        app.state::<PlanStore>(),
        app.state::<PtyManager>(),
    );
    let reg = registry(&store, root, team.id, &team.plan)?;
    for (agent, wt) in &team.worktrees {
        let key = (team.id, agent.clone());
        let orphan = reg.status(agent).map_err(|e| e.to_string())? == Some(TaskStatus::Running)
            && !plans
                .agent_sessions(team.id, agent)
                .iter()
                .any(|s| pty.is_running(s));
        if second_strike(suspects, key, orphan) {
            reg.finish(agent, &wt.path, "session ended")
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Whether `key` is an orphan this tick and was one the tick before (then forgets it).
fn second_strike<K: std::hash::Hash + Eq>(
    suspects: &mut std::collections::HashSet<K>,
    key: K,
    orphan: bool,
) -> bool {
    if !orphan {
        suspects.remove(&key);
        false
    } else if suspects.remove(&key) {
        true
    } else {
        suspects.insert(key);
        false
    }
}

/// Background tick: keep every open team moving (results, verifications, outbox) whether or
/// not its panels are open.
fn tick_loop(app: &tauri::AppHandle) {
    let (store, plans) = (app.state::<Store>(), app.state::<PlanStore>());
    let mut last_errors = std::collections::HashMap::<u64, String>::new();
    let mut last_tasks = HashMap::<u64, String>::new();
    let mut orphans = std::collections::HashSet::<(u64, String)>::new();
    loop {
        std::thread::sleep(TICK);
        // A panic in one tick is logged; the loop (timeouts, verifications, outbox) survives.
        let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            reap_timeouts(app);
            let workspaces = match store.list_workspaces() {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("tick: {e}");
                    return;
                }
            };
            for ws in workspaces {
                let teams = match plans.open_teams(&store, ws.id) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("tick: workspace {}: {e}", ws.id);
                        continue;
                    }
                };
                for team in teams {
                    if plans.is_cleaning(team.id) {
                        continue;
                    }
                    let result = finish_orphans(app, &team, &ws.root(), &mut orphans)
                        .and_then(|()| sync_tasks(app, team.id))
                        .and_then(|tasks| {
                            let (_, arrived) = sync_messages(app, team.id)?;
                            let snapshot =
                                serde_json::to_string(&tasks).map_err(|e| e.to_string())?;
                            let changed = last_tasks.get(&team.id) != Some(&snapshot);
                            last_tasks.insert(team.id, snapshot);
                            if changed || arrived > 0 {
                                // Open panels refresh now instead of waiting for their fallback poll.
                                // Carries the tasks so the task panel need not sync them again.
                                let payload = TeamChanged {
                                    plan_id: team.id,
                                    tasks: &tasks,
                                };
                                app.emit(TEAM_CHANGED, payload).map_err(|e| e.to_string())?;
                            }
                            Ok(())
                        });
                    match result {
                        // Logged once per distinct error, not every tick.
                        Err(e) if last_errors.get(&team.id) != Some(&e) => {
                            eprintln!("tick: team {}: {e}", team.id);
                            last_errors.insert(team.id, e);
                        }
                        Err(_) => {}
                        Ok(_) => {
                            last_errors.remove(&team.id);
                        }
                    }
                }
            }
        }));
        if tick.is_err() {
            eprintln!("tick panicked; continuing");
        }
    }
}

/// Event (payload: `TeamChanged`) emitted when a team's tasks or messages changed.
const TEAM_CHANGED: &str = "team-changed";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TeamChanged<'a> {
    plan_id: u64,
    /// The registry as the tick just synced it.
    tasks: &'a [TaskView],
}

const TICK: std::time::Duration = std::time::Duration::from_secs(5);

/// No PTY survives a restart: finish every task still marked running with the result it left,
/// or as lost; give teams approved before the registry existed their rows.
fn recover_lost_tasks(store: &Store, plans: &PlanStore) -> Result<(), String> {
    for ws in store.list_workspaces().map_err(|e| e.to_string())? {
        let root = ws.root();
        for team in plans.open_teams(store, ws.id).map_err(|e| e.to_string())? {
            let reg = registry(store, &root, team.id, &team.plan)?;
            if reg.tasks().map_err(|e| e.to_string())?.is_empty() {
                // Approved before the registry existed: start every agent as pending.
                let agents: Vec<&str> = team.plan.agents.iter().map(|a| a.id.as_str()).collect();
                reg.register(&agents).map_err(|e| e.to_string())?;
            }
            for (agent, wt) in &team.worktrees {
                if reg.status(agent).map_err(|e| e.to_string())? == Some(TaskStatus::Running) {
                    reg.finish(agent, &wt.path, "session lost (INA restarted)")
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(PtyManager::default())
        .manage(PlanStore::default())
        .manage(Deadlines::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(Store::open(&dir.join("ina.db"))?);
            // Before any command or the tick can run: a spawn racing this would be marked lost.
            if let Err(e) = recover_lost_tasks(&app.state::<Store>(), &app.state::<PlanStore>()) {
                eprintln!("recovering tasks failed: {e}");
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || tick_loop(&handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pty_spawn_shell,
            pty_spawn_agent,
            home_info,
            home_intent,
            workspace_create,
            plan_generate,
            plan_draft,
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
            team_archive,
            team_report,
            preview_page,
            team_resumable,
            team_list,
            team_tasks,
            claude_setup,
            usage_read,
            agent_activity,
            agent_send,
            team_messages,
            team_send,
            message_decide,
            team_preview,
            message_notify
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orphan_finished_only_on_second_tick_in_a_row() {
        let mut s = std::collections::HashSet::new();
        assert!(!second_strike(&mut s, "a", true));
        assert!(second_strike(&mut s, "a", true));
        assert!(
            !second_strike(&mut s, "a", true),
            "forgotten after finishing"
        );
        assert!(!second_strike(&mut s, "b", true));
        assert!(
            !second_strike(&mut s, "b", false),
            "session came up meanwhile"
        );
        assert!(!second_strike(&mut s, "b", true));
    }

    #[test]
    fn session_id_validation() {
        assert!(valid_id("agent-1_a").is_ok());
        for bad in ["", "../x", "a b", "a;b", &"x".repeat(65)] {
            assert!(valid_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn restart_finishes_orphaned_running_tasks() {
        let dir = std::env::temp_dir().join(format!("ina-recover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::in_memory().unwrap();
        let plans = PlanStore::default();
        let ws = store.add_workspace(&dir).unwrap();
        let plan: Plan = serde_json::from_value(serde_json::json!({
            "summary": "g", "mode": "parallel",
            "agents": [
                {"id": "a", "name": "A", "role": "r", "scope": "s", "task": "t",
                 "acceptance": ["x"], "dependsOn": []},
                {"id": "b", "name": "B", "role": "r", "scope": "s", "task": "t",
                 "acceptance": ["x"], "dependsOn": []}
            ]
        }))
        .unwrap();
        let id = plans.insert(&store, ws.id, &plan).unwrap();
        let wt = |n: &str| worktree::Worktree {
            path: dir.join(n),
            branch: format!("ina/r/{n}"),
        };
        let map = [("a".to_string(), wt("a")), ("b".to_string(), wt("b"))]
            .into_iter()
            .collect();
        plans.approve(&store, id, &map).unwrap();
        let root = ws.root();
        let reg = registry(&store, &root, id, &plan).unwrap();
        reg.register(&["a", "b"]).unwrap();
        reg.transition("a", TaskStatus::Running, None, None)
            .unwrap();
        std::fs::create_dir_all(dir.join("b/.ina")).unwrap();
        std::fs::write(
            dir.join("b/.ina/result.json"),
            r#"{"status":"done","summary":"ok"}"#,
        )
        .unwrap();
        reg.transition("b", TaskStatus::Running, None, None)
            .unwrap();

        recover_lost_tasks(&store, &plans).unwrap();
        let tasks = store.tasks(reg.team).unwrap();
        assert_eq!(tasks[0].status, "failed");
        assert!(
            tasks[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("INA restarted")
        );
        assert_eq!(
            (tasks[1].status.as_str(), tasks[1].summary.as_deref()),
            ("verifying", Some("ok"))
        );

        // After verification, running again clears the previous run's summary.
        reg.transition("b", TaskStatus::Done, Some("ok"), None)
            .unwrap();
        reg.transition("b", TaskStatus::Running, None, None)
            .unwrap();
        assert_eq!(store.tasks(reg.team).unwrap()[1].summary, None);

        // A team approved before the registry existed gets pending rows on startup.
        let legacy = plans.insert(&store, ws.id, &plan).unwrap();
        plans.approve(&store, legacy, &map).unwrap();
        recover_lost_tasks(&store, &plans).unwrap();
        let rows = store.tasks(to_team(legacy).unwrap()).unwrap();
        assert_eq!(
            rows.iter()
                .map(|t| (t.agent_id.as_str(), t.status.as_str()))
                .collect::<Vec<_>>(),
            [("a", "pending"), ("b", "pending")]
        );
    }
}
