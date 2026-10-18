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
