import { describe, expect, it } from "vitest";
import { addHomeItem, newHomeConversation, setHome, useHome } from "./homeStore";

describe("homeStore", () => {
  it("keeps the conversation and draft in localStorage until a new conversation", () => {
    addHomeItem({ kind: "user", text: "oi" });
    setHome({ draft: "rascunho", busy: "Pensando…" });
    const saved = JSON.parse(localStorage.getItem("ina.home") ?? "{}") as Record<string, unknown>;
    expect(saved).toEqual({ items: [{ kind: "user", text: "oi" }], draft: "rascunho" });
    newHomeConversation();
    expect(JSON.parse(localStorage.getItem("ina.home") ?? "{}")).toEqual({ items: [], draft: "" });
    expect(typeof useHome).toBe("function");
  });
});
