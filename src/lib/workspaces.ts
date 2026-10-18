import { invoke } from "@tauri-apps/api/core";

export interface Workspace {
  id: number;
  name: string;
  path: string;
}

export const listWorkspaces = () => invoke<Workspace[]>("workspace_list");
export const addWorkspace = (path: string) => invoke<Workspace>("workspace_add", { path });
export const removeWorkspace = (id: number) => invoke<null>("workspace_remove", { id });
