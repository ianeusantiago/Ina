import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

const { transitions } = await import("./notify");

describe("transitions", () => {
  it("notifies changes to done/failed/blocked, not the first sighting", () => {
    const seen = new Map();
    const t = (status: string) => [{ agentId: "a", status }] as never;
    expect(transitions(seen, 1, t("done"))).toEqual([]);
    expect(transitions(seen, 1, t("done"))).toEqual([]);
    expect(transitions(seen, 1, t("running"))).toEqual([]);
    expect(transitions(seen, 1, t("failed"))).toEqual([{ agentId: "a", status: "failed" }]);
    expect(transitions(seen, 2, t("done"))).toEqual([]);
  });
});
