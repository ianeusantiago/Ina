import type { Locator } from "@playwright/test";
import { expect, test } from "./fixtures";

// Full team flow with real Claude Code sessions (costs tokens). Opt-in:
//   INA_E2E_WORKSPACE=<name of a workspace already added in INA: a git repo with a commit,
//   trusted in Claude Code> npm run e2e
const workspace = process.env.INA_E2E_WORKSPACE;

test("plan, approve, verify and preview integration", async ({ app }) => {
  test.skip(!workspace, "set INA_E2E_WORKSPACE to run the real team flow");
  test.setTimeout(10 * 60_000);
  await app.getByRole("combobox").first().selectOption("en");
  await app.getByRole("button", { name: workspace ?? "", exact: true }).click();
  await app
    .getByRole("textbox", { name: "Describe the task and the team" })
    .fill(
      "Exactly one agent (id code-e2e): create e2e_mod.py with twice(x) returning 2*x and " +
        "test_e2e_mod.py with unittest tests, then commit. Verification must be " +
        "python -m unittest test_e2e_mod. Be minimal.",
    );
  await app.getByRole("button", { name: "Generate plan" }).click();
  const approve = app.getByRole("button", { name: /^Approve/ });
  await expect(approve).toBeVisible({ timeout: 5 * 60_000 });
  await expect(app.locator("code", { hasText: "python -m unittest test_e2e_mod" })).toBeVisible();
  await approve.click();
  await app.getByRole("radio", { name: "Terminals" }).click();

  // Exact heading: "Tasks" alone would also match the Interface ("Task" + "Single…").
  const tasks = app.locator("section", { has: app.getByText("Tasks", { exact: true }) }).first();
  await expect(tasks).toContainText("verified: 1 check(s) passed", { timeout: 8 * 60_000 });
  // Every session (orchestrator included) started.
  await expect(app.getByText(/Failed to start session/)).toHaveCount(0);

  // The interface summarizes what the agent's Claude Code session did.
  // Members are listed under the workspace in the sidebar (newest team first, open).
  await app.locator("aside li li ul").first().getByRole("button").nth(1).click();
  await expect(app.getByRole("radio", { name: "Interface" })).toBeChecked();
  // The agent's reply from its Claude Code session shows as a chat bubble.
  await expect(app.locator('[data-role="agent"]').first()).toBeVisible({ timeout: 15_000 });
  // Typing in the interface reaches the agent, and its answer comes back here.
  await app.getByRole("textbox", { name: /^Message / }).fill("Reply with only the word pong.");
  await app.getByRole("button", { name: "Send", exact: true }).click();
  await expect(app.locator('[data-role="user"]', { hasText: "pong" }).first()).toBeVisible();
  await expect(app.locator('[data-role="agent"]', { hasText: /pong/i }).last()).toBeVisible({
    timeout: 3 * 60_000,
  });
  // Switching views keeps the selected member.
  await app.getByRole("radio", { name: "Terminals" }).click();
  await app.getByRole("radio", { name: "Interface" }).click();
  await expect(app.locator('[data-role="agent"]', { hasText: /pong/i }).last()).toBeVisible();
  await app.getByRole("radio", { name: "Terminals" }).click();

  await app.getByRole("button", { name: "Integrate", exact: true }).click();
  await expect(app.getByText("Preview (nothing was changed):")).toBeVisible();
  await expect(app.getByText("clean merge")).toBeVisible();
  await app.getByRole("button", { name: "Cancel", exact: true }).click();
});

async function box(l: Locator) {
  const b = await l.boundingBox();
  if (!b) throw new Error("element not rendered");
  return b;
}

test("resizes the task area by dragging and with the keyboard", async ({ app }) => {
  test.skip(!workspace, "set INA_E2E_WORKSPACE (with an open team) to run");
  await app.getByRole("combobox").first().selectOption("en");
  await app.getByRole("button", { name: workspace ?? "", exact: true }).click();
  await app.getByRole("button", { name: "Resume" }).first().click();
  await app.getByRole("radio", { name: "Terminals" }).click();
  const handle = app.getByRole("separator", { name: /Resize panels/ });
  const area = handle.locator("xpath=preceding-sibling::div[1]");
  await handle.focus();
  for (let i = 0; i < 15; i++) await app.keyboard.press("ArrowUp"); // start at the minimum
  const before = (await box(area)).height;
  const h = await box(handle);
  await app.mouse.move(h.x + h.width / 2, h.y + 2);
  await app.mouse.down();
  await app.mouse.move(h.x + h.width / 2, h.y + 120, { steps: 5 });
  await app.mouse.up();
  const dragged = (await box(area)).height;
  expect(dragged).toBeGreaterThan(before + 60);
  await handle.focus();
  await app.keyboard.press("ArrowUp");
  expect((await box(area)).height).toBeLessThan(dragged);
});
