import { Channel, invoke } from "@tauri-apps/api/core";

export type PtyMessage =
  { type: "output"; data: number[] } | { type: "exited"; code: number | null };

export function spawnShell(
  workspaceId: number,
  id: string,
  cols: number,
  rows: number,
  onEvent: (msg: PtyMessage) => void,
): Promise<null> {
  const channel = new Channel<PtyMessage>();
  channel.onmessage = onEvent;
  return invoke<null>("pty_spawn_shell", { workspaceId, id, cols, rows, onEvent: channel });
}

export const writePty = (id: string, data: string) => invoke<null>("pty_write", { id, data });
export const resizePty = (id: string, cols: number, rows: number) =>
  invoke<null>("pty_resize", { id, cols, rows });
export const killPty = (id: string) => invoke<null>("pty_kill", { id });
