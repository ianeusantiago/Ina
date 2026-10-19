import { describe, expect, it } from "vitest";
import { parsePermission } from "./permission";

// Captured from Claude Code v2.1 running in INA's terminal.
const SCREEN = `❯ Create a file named ina-perm-2.txt containing hi
● Write(ina-perm-2.txt)
─────────────────────────────────────────────────
 Create file
 ina-perm-2.txt
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
  1 hi
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Do you want to create ina-perm-2.txt?
 ❯ 1. Yes
   2. Yes, and switch to accept edits (auto-approve file edits and common file commands)
      for this session (shift+tab)
   3. No
 Esc to cancel · Tab to amend`.split("\n");

describe("parsePermission", () => {
  it("reads an active menu", () => {
    const p = parsePermission(SCREEN);
    expect(p?.question).toBe("Do you want to create ina-perm-2.txt?");
    expect(p?.action).toBe("Write(ina-perm-2.txt)");
    expect(p?.detail.split("\n")).toEqual(["Create file", " ina-perm-2.txt", "  1 hi"]);
    expect(p?.options.map((o) => [o.key, o.kind])).toEqual([
      ["1", "allow"],
      ["2", "always"],
      ["3", "deny"],
    ]);
    expect(p?.options[1]?.label).toContain("for this session");
  });

  it("ignores an answered menu (no footer) and plain output", () => {
    expect(parsePermission(SCREEN.slice(0, -1))).toBeNull();
    expect(parsePermission(["● Write(x)", "  ⎿  User rejected write to x"])).toBeNull();
  });
});
