import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { expect, test } from "./fixtures";

test("switches language and keeps the choice", async ({ app }) => {
  const lang = app.getByRole("combobox").first();
  const original = await lang.inputValue();
  await lang.selectOption("en");
  await expect(app.getByText("Language")).toBeVisible();
  await app.reload();
  await expect(app.getByRole("combobox").first()).toHaveValue("en");
  await app.getByRole("combobox").first().selectOption("pt-BR");
  await expect(app.getByText("Idioma")).toBeVisible();
  await app.getByRole("combobox").first().selectOption(original);
});

test("adds a workspace, runs a real shell in it, removes it", async ({ app }) => {
  await app.getByRole("combobox").first().selectOption("en");
  const dir = mkdtempSync(join(tmpdir(), "ina-e2e-"));
  const name = basename(dir);
  await app.locator("aside form input").fill(dir);
  await app.getByRole("button", { name: "Add", exact: true }).click();
  const entry = app.getByRole("button", { name, exact: true });
  await expect(entry).toBeVisible();

  // The workspace opens with a real PowerShell session in that folder.
  const term = app.locator("section", { has: app.locator(".xterm") }).first();
  await expect(term.getByRole("status")).toHaveText("Running");
  await term.locator(".xterm").click();
  await app.keyboard.type("echo ina-e2e-$((2+3))\r");
  await expect(term.locator(".xterm-rows")).toContainText("ina-e2e-5");

  await app.getByRole("button", { name: `Remove ${name}` }).click();
  await expect(entry).toHaveCount(0);
});

test("rejects a path that is not a directory", async ({ app }) => {
  await app.getByRole("combobox").first().selectOption("en");
  await app.locator("aside form input").fill(join(tmpdir(), "ina-e2e-missing-xyz"));
  await app.getByRole("button", { name: "Add", exact: true }).click();
  await expect(app.getByRole("alert")).toContainText("not a directory");
});
