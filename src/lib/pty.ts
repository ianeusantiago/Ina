import { Channel, invoke } from "@tauri-apps/api/core";

export type PtyMessage =
  { type: "output"; data: number[] } | { type: "exited"; code: number | null };

export type Spawner = (
  id: string,
  cols: number,
  rows: number,
  onEvent: (msg: PtyMessage) => void,
) => Promise<null>;

function channelFor(onEvent: (msg: PtyMessage) => void): Channel<PtyMessage> {
  const channel = new Channel<PtyMessage>();
  channel.onmessage = onEvent;
  return channel;
}

export function spawnShell(
  workspaceId: number,
  id: string,
  cols: number,
  rows: number,
  onEvent: (msg: PtyMessage) => void,
): Promise<null> {
  return invoke<null>("pty_spawn_shell", {
    workspaceId,
    id,
    cols,
    rows,
    onEvent: channelFor(onEvent),
  });
}

export const writePty = (id: string, data: string) => invoke<null>("pty_write", { id, data });
export const resizePty = (id: string, cols: number, rows: number) =>
  invoke<null>("pty_resize", { id, cols, rows });
export const killPty = (id: string) => invoke<null>("pty_kill", { id });

export function spawnAgent(
  planId: number,
  agentId: string,
  id: string,
  cols: number,
  rows: number,
  onEvent: (msg: PtyMessage) => void,
): Promise<null> {
  return invoke<null>("pty_spawn_agent", {
    planId,
    agentId,
    id,
    cols,
    rows,
    onEvent: channelFor(onEvent),
  });
}
