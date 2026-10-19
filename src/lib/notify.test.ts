import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

const { transitions } = await import("./notify");

describe("transitions", () => {
  const at = (status: string, updatedAt: string) => [{ agentId: "a", status, updatedAt }] as never;
  const since = Date.parse("2026-10-19T12:00:00Z");

  it("notifies changes to done/failed/blocked", () => {
    const seen = new Map();
    expect(transitions(seen, 1, at("running", "2026-10-19 12:01:00"), since)).toEqual([]);
    expect(transitions(seen, 1, at("done", "2026-10-19 12:02:00"), since)).toEqual([
      { agentId: "a", status: "done" },
    ]);
    expect(transitions(seen, 1, at("done", "2026-10-19 12:02:00"), since)).toEqual([]);
  });

  it("first sighting: only results newer than the app start", () => {
    const seen = new Map();
    expect(transitions(seen, 1, at("done", "2026-10-19 11:00:00"), since)).toEqual([]);
    expect(transitions(seen, 2, at("failed", "2026-10-19 12:05:00"), since)).toEqual([
      { agentId: "a", status: "failed" },
    ]);
  });
});
