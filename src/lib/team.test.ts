import { describe, expect, it } from "vitest";
import { prerequisites, type AgentSpec, type Plan } from "./team";

const agent = (id: string, dependsOn: string[] = []): AgentSpec => ({
  id,
  name: id,
  role: "r",
  scope: "s",
  task: "t",
  acceptance: [],
  dependsOn,
  verify: [],
});

describe("prerequisites", () => {
  const agents = [agent("ui", ["api"]), agent("docs"), agent("api")];

  it("parallel: only declared dependencies", () => {
    const p: Plan = { summary: "g", mode: "parallel", agents };
    expect(prerequisites(p, "ui")).toEqual(["api"]);
    expect(prerequisites(p, "docs")).toEqual([]);
  });

  it("sequential: everyone earlier in dependency order", () => {
    const p: Plan = { summary: "g", mode: "sequential", agents };
    expect(prerequisites(p, "docs")).toEqual([]);
    expect(prerequisites(p, "api")).toEqual(["docs"]);
    expect(prerequisites(p, "ui")).toEqual(["docs", "api"]);
  });
});
