import { invoke } from "@tauri-apps/api/core";
import type { Workspace } from "./workspaces";

export interface Intent {
  action: "create" | "answer";
  workspaceName: string;
  task: string;
  reply: string;
}

export interface HomeBase {
  path: string;
  trusted: boolean;
}

export const homeInfo = () => invoke<HomeBase>("home_info");
export const homeIntent = (request: string) => invoke<Intent>("home_intent", { request });
export const createWorkspace = (name: string) => invoke<Workspace>("workspace_create", { name });
