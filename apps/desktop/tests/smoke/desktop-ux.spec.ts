import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
});

for (const theme of ["light", "dark"]) {
  test(`${theme}: resizing preserves the request and keeps shell controls in view`, async ({ page }, testInfo) => {
    await page.addInitScript((mode) => localStorage.setItem("unfour.theme", mode), theme);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    const url = page.getByRole("textbox", { name: "Request URL", exact: true });
    await url.fill("https://example.invalid/retained-draft");
    await expect(page.getByRole("separator", { name: /Resize horizontal split/ })).toBeVisible();
    await page.setViewportSize({ width: 960, height: 600 });
    await expect(page.getByRole("separator", { name: /Resize vertical split/ })).toBeVisible();
    await expect(url).toHaveValue("https://example.invalid/retained-draft");
    const status = page.locator(".u-statusbar");
    await expect(status).toBeInViewport({ ratio: 1 });
    await expect(page.getByRole("button", { name: "Toggle inspector" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Toggle bottom panel" })).toHaveCount(0);
    await expect(status).not.toContainText("Connected");
    await page.screenshot({ path: testInfo.outputPath(`${theme}-960x600.png`) });
    await page.setViewportSize({ width: 1440, height: 900 });
    await expect(page.getByRole("separator", { name: /Resize horizontal split/ })).toBeVisible();
    await expect(url).toHaveValue("https://example.invalid/retained-draft");
    await page.screenshot({ path: testInfo.outputPath(`${theme}-1440x900.png`) });
  });
}

test("command search, keyboard selection, and focus return work end to end", async ({ page }) => {
  await page.goto("/");
  const url = page.getByRole("textbox", { name: "Request URL", exact: true });
  await url.click();
  await page.keyboard.press("Control+Shift+P");
  const search = page.getByRole("combobox", { name: "Search commands" });
  await expect(search).toBeFocused();
  await search.fill("unmatched command");
  await expect(page.getByText("No matching commands")).toBeVisible();
  await search.press("Escape");
  await expect(url).toBeFocused();
  await page.keyboard.press("Control+Shift+P");
  await expect(search).toHaveValue("");
  await search.fill("ssh");
  await expect(page.getByRole("option")).toHaveCount(1);
  await search.press("Enter");
  await expect(page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Toggle bottom panel" })).toBeVisible();
});

test("reopening variables preserves a draft and module navigation honors Cancel and Discard", async ({ page }) => {
  await page.goto("/");
  const environment = page.getByRole("button", { name: "Active environment" });
  await environment.click();
  await page.getByRole("menuitem", { name: "Manage variables" }).click();
  const name = page.getByRole("textbox", { name: "Name", exact: true });
  await name.fill("Unsaved environment draft");
  await environment.click();
  await page.getByRole("menuitem", { name: "Manage variables" }).click();
  await expect(name).toHaveValue("Unsaved environment draft");
  const nav = page.getByRole("navigation", { name: "Modules" });
  await expect(nav.locator('[aria-current="page"]')).toHaveCount(0);
  await page.keyboard.press("Control+Shift+P");
  const search = page.getByRole("combobox", { name: "Search commands" });
  await search.fill("ssh");
  await search.press("Enter");
  const confirmation = page.getByRole("dialog", { name: "Discard workspace variable changes?" });
  await expect(confirmation).toBeVisible();
  await confirmation.getByRole("button", { name: "Cancel" }).click();
  await expect(name).toHaveValue("Unsaved environment draft");
  await nav.getByRole("button", { name: "SSH Terminal" }).click();
  await confirmation.getByRole("button", { name: "Discard changes" }).click();
  await expect(nav.getByRole("button", { name: "SSH Terminal" })).toHaveAttribute("aria-current", "page");
  await expect(name).toHaveCount(0);
});
