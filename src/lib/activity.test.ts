import { describe, expect, it } from "vitest";
import { baseName, conversation, type ActivityEvent } from "./activity";

const tool = (detail: string): ActivityEvent => ({
  kind: "tool",
  tool: "edit",
  detail,
  added: 1,
  removed: 0,
});

describe("conversation", () => {
  it("drops the task prompt and tool noise, keeps the chat", () => {
    const c = conversation([
      { kind: "user", text: "You are agent ..." },
      tool("src/a.ts"),
      { kind: "assistant", text: "Done." },
      { kind: "user", text: "hello" },
      { kind: "assistant", text: "Hi!" },
    ]);
    expect(c.messages).toEqual([
      { role: "agent", text: "Done." },
      { role: "user", text: "hello" },
      { role: "agent", text: "Hi!" },
    ]);
    expect(c.working).toBe(false);
    expect(c.current).toBeNull();
    expect(c.sent.get("hello")).toBe(1);
  });

  it("is working after a message or a tool use", () => {
    expect(conversation([{ kind: "user", text: "task" }]).working).toBe(true);
    const c = conversation([{ kind: "user", text: "task" }, tool("C:\\w\\index.html")]);
    expect(c.working).toBe(true);
    expect(c.current?.detail).toBe("C:\\w\\index.html");
    expect(baseName(c.current?.detail ?? "")).toBe("index.html");
    expect(conversation([]).working).toBe(false);
  });
});
