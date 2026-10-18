import { invoke } from "@tauri-apps/api/core";

export interface AgentSpec {
  id: string;
  name: string;
  role: string;
  scope: string;
  task: string;
  acceptance: string[];
  dependsOn: string[];
  /** Verification commands, approved with the plan. */
  verify: Check[];
}

export interface Check {
  program: "cargo" | "node" | "python" | "pytest";
  args: string[];
}

export const checkText = (c: Check) => [c.program, ...c.args].join(" ");

export interface Plan {
  summary: string;
  mode: "parallel" | "sequential";
  agents: AgentSpec[];
}

export interface PlanDraft {
  id: number;
  plan: Plan;
}

export const ORCHESTRATOR_ID = "orchestrator";

export const generatePlan = (workspaceId: number, request: string) =>
  invoke<PlanDraft>("plan_generate", { workspaceId, request });
export const approvePlan = (planId: number) => invoke<null>("plan_approve", { planId });
export const workspaceTrusted = (workspaceId: number) =>
  invoke<boolean>("workspace_trusted", { workspaceId });

export interface BranchStatus {
  branch: string;
  ahead: number;
  dirty: boolean;
  untracked: string[];
  files: string[];
}

export interface AgentBranch {
  agentId: string;
  status: BranchStatus;
}

export type IntegrateStep =
  | { kind: "merged"; agentId: string }
  | { kind: "skipped"; agentId: string }
  | { kind: "conflict"; agentId: string; files: string[] }
  | { kind: "failed"; agentId: string; error: string };

export interface CleanupResult {
  agentId: string;
  error: string | null;
}

export const teamStatus = (planId: number) => invoke<AgentBranch[]>("team_status", { planId });
export type PreviewStep =
  | { kind: "clean"; agentId: string }
  | { kind: "skipped"; agentId: string }
  | { kind: "conflict"; agentId: string; files: string[] }
  | { kind: "notReached"; agentId: string };

export interface IntegrationPreview {
  steps: PreviewStep[];
  overlaps: { file: string; agents: string[] }[];
}

export const teamPreview = (planId: number) =>
  invoke<IntegrationPreview>("team_preview", { planId });
export const teamIntegrate = (planId: number) =>
  invoke<IntegrateStep[]>("team_integrate", { planId });
export const teamCleanup = (planId: number, discardUntracked = false) =>
  invoke<CleanupResult[]>("team_cleanup", { planId, discardUntracked });
/** Agents that must be done before `id` may start (mirrors `Plan::prerequisites`). */
export function prerequisites(plan: Plan, id: string): string[] {
  if (plan.mode === "parallel") {
    return plan.agents.find((a) => a.id === id)?.dependsOn ?? [];
  }
  const done = new Set<string>();
  const order: string[] = [];
  while (order.length < plan.agents.length) {
    const next = plan.agents.find((a) => !done.has(a.id) && a.dependsOn.every((d) => done.has(d)));
    if (!next) break;
    done.add(next.id);
    order.push(next.id);
  }
  const i = order.indexOf(id);
  return i < 0 ? [] : order.slice(0, i);
}

export interface ClaudeSetup {
  installed: boolean;
  version: string | null;
  onboarded: boolean;
  loggedIn: boolean;
  error: string | null;
}

export const claudeSetup = () => invoke<ClaudeSetup>("claude_setup");

export type TaskStatus =
  "pending" | "running" | "verifying" | "done" | "failed" | "blocked" | "cancelled";

export interface TaskView {
  agentId: string;
  status: TaskStatus;
  summary: string | null;
  detail: string | null;
  updatedAt: string;
  resultError: string | null;
}

export const teamTasks = (planId: number) => invoke<TaskView[]>("team_tasks", { planId });
export interface MessageView {
  id: number;
  from: string;
  to: string | null;
  kind: string;
  text: string;
  status: "received" | "proposed" | "delivered" | "rejected";
  replyTo: number | null;
  at: string;
}

export const teamMessages = (planId: number) => invoke<MessageView[]>("team_messages", { planId });
export const sendMessage = (planId: number, to: string, text: string, replyTo: number | null) =>
  invoke<MessageView[]>("team_send", { planId, to, text, replyTo });
export const decideMessage = (planId: number, messageId: number, approve: boolean) =>
  invoke<MessageView[]>("message_decide", { planId, messageId, approve });

export const listTeams = (workspaceId: number) => invoke<PlanDraft[]>("team_list", { workspaceId });
