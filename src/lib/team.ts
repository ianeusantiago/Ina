import { invoke } from "@tauri-apps/api/core";

export interface AgentSpec {
  id: string;
  name: string;
  role: string;
  scope: string;
  task: string;
  acceptance: string[];
  dependsOn: string[];
}

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
  | { kind: "conflict"; agentId: string; files: string[] };

export interface CleanupResult {
  agentId: string;
  error: string | null;
}

export const teamStatus = (planId: number) => invoke<AgentBranch[]>("team_status", { planId });
export const teamIntegrate = (planId: number) =>
  invoke<IntegrateStep[]>("team_integrate", { planId });
export const teamCleanup = (planId: number, discardUntracked = false) =>
  invoke<CleanupResult[]>("team_cleanup", { planId, discardUntracked });
export const listTeams = (workspaceId: number) => invoke<PlanDraft[]>("team_list", { workspaceId });
