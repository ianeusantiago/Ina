import { invoke } from "@tauri-apps/api/core";

export interface UsageWindow {
  /** Window length in minutes (300 = 5-hour session, 10080 = week). */
  minutes: number;
  usedPercent: number;
  /** Unix seconds. */
  resetsAt: number | null;
}

export interface Usage {
  windows: UsageWindow[];
  updatedAt: number;
}

export interface UsageReport {
  /** Last limits a Claude session started by INA reported; null before the first one. */
  claude: Usage | null;
  codex: { Ok: Usage } | { Err: string };
}

export const readUsage = () => invoke<UsageReport>("usage_read");
