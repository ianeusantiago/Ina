import { describe, expect, it } from "vitest";
import type { TaskView } from "../lib/team";
import { progress } from "./TeamProgress";

const task = (agentId: string, status: TaskView["status"]) => ({ agentId, status }) as TaskView;

describe("progress", () => {
  it("moves through the stages", () => {
    expect(progress([task("a", "pending"), task("b", "pending")], 2, false)).toMatchObject({
      stage: 0,
      pct: 0,
    });
    expect(progress([task("a", "done"), task("b", "running")], 2, false)).toMatchObject({
      stage: 1,
      pct: 38,
      done: 1,
    });
    expect(progress([task("a", "done"), task("b", "done")], 2, false)).toMatchObject({
      stage: 2,
      pct: 75,
    });
    expect(progress([task("a", "done"), task("b", "done")], 2, true)).toMatchObject({
      stage: 3,
      pct: 100,
    });
    expect(progress([task("a", "failed")], 1, false).problem).toBe(true);
  });
});
