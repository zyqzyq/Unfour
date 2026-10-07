import { expect, test } from "@playwright/test";

test("Monaco initializes and edits with all external network requests blocked", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("unfour.locale", "en");
  });
  const external: string[] = [];
  await page.route("**/*", (route) => {
    const url = new URL(route.request().url());
    if (["127.0.0.1", "localhost"].includes(url.hostname)) return route.continue();
    external.push(url.href);
    return route.abort();
  });
  await page.goto("/");
  const modules = page.getByRole("navigation", { name: "Modules" });
  await modules.getByRole("button", { name: "API", exact: true }).click();
  await page.getByRole("button", { name: "Scripts", exact: true }).click();
  const editor = page.locator(".monaco-editor").first();
  await expect(editor).toBeVisible();
  await editor.click();
  await page.keyboard.type("const offline = true;");
  await expect(editor).toContainText("const offline = true;");
  expect(external).toEqual([]);
});
