import { test as base, chromium, type Page } from "@playwright/test";

const CDP = process.env.INA_CDP ?? "http://127.0.0.1:9222";

/** The running INA window, attached over CDP. */
export const test = base.extend<{ app: Page }>({
  // eslint-disable-next-line no-empty-pattern
  app: async ({}, provide) => {
    const browser = await chromium.connectOverCDP(CDP);
    const page = browser.contexts()[0]?.pages()[0];
    if (!page) throw new Error(`no INA page at ${CDP}; is the app running with CDP enabled?`);
    await page.reload();
    const lang = page.getByRole("combobox").first();
    await lang.waitFor();
    const original = await lang.inputValue();
    await provide(page);
    await page.getByRole("combobox").first().selectOption(original);
    await browser.close(); // detaches; the app keeps running
  },
});

export { expect } from "@playwright/test";
