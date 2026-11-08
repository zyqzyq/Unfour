import { expect, test, type Locator, type Page } from "@playwright/test";

// Browser command-bus fixtures only. No native database, SSH, or remote Flow
// execution is involved; saved definitions live in this page's mock store.
async function prepareWorkbench(page: Page, theme: "light" | "dark") {
  await page.addInitScript((mode) => {
    if (!localStorage.getItem("unfour.locale")) localStorage.setItem("unfour.locale", "en");
    if (!localStorage.getItem("unfour.theme")) localStorage.setItem("unfour.theme", mode);
  }, theme);
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockStore.savedRequests.push({
        id: 'redesign-health', workspaceId: 'mock-workspace', name: 'Health endpoint',
        method: 'GET', url: 'https://example.test/redesign-health',
        headersJson: '[]', queryJson: '[]', bodyKind: 'none', body: '', deletedAt: null
      });
    ` });
  });
}

async function expectNoHorizontalOverflow(page: Page) {
  await expect.poll(() => page.evaluate(() =>
    document.documentElement.scrollWidth <= window.innerWidth,
  )).toBe(true);
}

async function expectReachable(control: Locator) {
  await control.scrollIntoViewIfNeeded();
  await expect(control).toBeInViewport({ ratio: 1 });
  await expect(control).toBeEnabled();
  // Actionability also checks overlays and clipping, beyond DOM visibility.
  await control.click({ trial: true });
}

async function expectFlowPanelsDoNotOverlap(page: Page) {
  await expect.poll(() => page.locator(".flow-workbench").evaluate((workbench) => {
    const graph = workbench.querySelector(".react-flow")!.getBoundingClientRect();
    const help = workbench.querySelector(".flow-canvas-help")!.getBoundingClientRect();
    const inspector = workbench.querySelector(".flow-inspector")!.getBoundingClientRect();
    const stacked = getComputedStyle(workbench).flexDirection === "column";
    return graph.top >= help.bottom - 1 && (!stacked || graph.bottom <= inspector.top + 1);
  })).toBe(true);
}

async function createFlow(page: Page) {
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await page.getByRole("textbox", { name: "Flow name", exact: true }).fill("Service health check");
  await expect(page.getByLabel("Flow Canvas", { exact: true })).toBeVisible();
  const inspector = page.getByRole("complementary", { name: "Inspector", exact: true });
  await inspector.getByRole("button", { name: "Add input", exact: true }).click();
  await inspector.getByRole("textbox", { name: "Input name", exact: true }).fill("ready");
  await inspector.getByRole("combobox", { name: "Type", exact: true }).selectOption("boolean");
  await page.getByRole("group", { name: "API Request", exact: true }).click();
  await inspector.getByLabel("Referenced resource", { exact: true }).selectOption("redesign-health");
  await inspector.getByRole("textbox", { name: "Step name", exact: true }).fill("Check service");
  return inspector;
}

for (const theme of ["light", "dark"] as const) {
  test(`${theme}: populated Flow retains inspector edits and saved revisions`, async ({ page }, testInfo) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await prepareWorkbench(page, theme);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");
    const inspector = await createFlow(page);
    await page.getByRole("button", { name: "Insert node · Check service → End", exact: true }).click();
    await page.getByRole("menuitem", { name: "Condition", exact: true }).click();
    await inspector.getByRole("textbox", { name: "Step name", exact: true }).fill("Choose path");
    await page.getByRole("button", { name: "Fit view", exact: true }).click();
    await expect(page.locator(".react-flow__node")).toHaveCount(4);
    const save = page.getByRole("button", { name: "Save", exact: true });
    await expectReachable(save);
    await save.click();
    await expect(page.getByText("r1", { exact: true })).toBeVisible();
    await expectFlowPanelsDoNotOverlap(page);
    await expectNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath(`flow-canvas-${theme}.png`), animations: "disabled" });

    await page.getByRole("group", { name: "Check service", exact: true }).click();
    await expect(inspector.getByLabel("Referenced resource", { exact: true })).toHaveValue("redesign-health");
    await inspector.getByRole("textbox", { name: "Step name", exact: true }).fill("Check API status");
    await save.click();
    await expect(page.getByText("r2", { exact: true })).toBeVisible();
    await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "API Client", exact: true }).click();
    await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow", exact: true }).click();
    await expect(page.getByRole("textbox", { name: "Flow name", exact: true })).toHaveValue("Service health check");
    await expect(page.getByRole("group", { name: "Check API status", exact: true })).toBeVisible();
    await expect(page.getByText("r2", { exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath(`flow-inspector-${theme}.png`), animations: "disabled" });
    expect(errors).toEqual([]);
  });
}

for (const viewport of [{ width: 1100, height: 720 }, { width: 960, height: 600 }]) {
  test(`${viewport.width}x${viewport.height}: API, Flow and settings controls remain reachable`, async ({ page }, testInfo) => {
    await prepareWorkbench(page, "dark");
    await page.setViewportSize(viewport);
    await page.route("https://example.test/redesign-health", (route) => route.fulfill({
      contentType: "application/json", body: JSON.stringify({ service: "unfour", healthy: true }),
    }));
    await page.goto("/");
    const url = page.getByRole("textbox", { name: "Request URL", exact: true });
    await expectReachable(url);
    await url.fill("https://example.test/redesign-health");
    const send = page.getByRole("button", { name: "Send", exact: true }).first();
    await expectReachable(send);
    await send.click();
    await expect(page.locator(".monaco-editor:visible")).toBeVisible();
    await expectNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath("api-compact.png"), animations: "disabled" });

    const inspector = await createFlow(page);
    const stepName = inspector.getByRole("textbox", { name: "Step name", exact: true });
    await expectReachable(stepName);
    await stepName.fill("Compact viewport edit");
    const save = page.getByRole("button", { name: "Save", exact: true });
    await expectReachable(save);
    await save.click();
    await expect(page.getByText("r1", { exact: true })).toBeVisible();
    await expectFlowPanelsDoNotOverlap(page);
    const advanced = page.locator(".flow-canvas-advanced > summary");
    await advanced.click();
    await expectReachable(page.getByRole("combobox", { name: "Insert on connection", exact: true }));
    await expectFlowPanelsDoNotOverlap(page);
    await advanced.click();
    await expectReachable(stepName);
    const history = page.getByRole("button", { name: "Run history", exact: true });
    await expectReachable(history);
    await history.click();
    await expect(page.getByText("No runs yet.", { exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expectNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath("flow-compact.png"), animations: "disabled" });

    const settings = page.locator(".app-shell > header").getByRole("button", { name: "Settings", exact: true });
    await expectReachable(settings);
    await settings.click();
    const theme = page.getByRole("dialog").getByRole("combobox", { name: "Theme", exact: true });
    await expectReachable(theme);
    await theme.selectOption("light");
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await expectNoHorizontalOverflow(page);
    await page.screenshot({ path: testInfo.outputPath("settings-compact.png"), animations: "disabled" });
    await page.keyboard.press("Escape");
    await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "API Client", exact: true }).click();
    await expect(url).toHaveValue("https://example.test/redesign-health");
  });
}

test("Chinese settings persist locale and theme, including live system appearance", async ({ page }, testInfo) => {
  await prepareWorkbench(page, "light");
  await page.setViewportSize({ width: 1100, height: 720 });
  await page.goto("/");
  await page.locator(".app-shell > header").getByRole("button", { name: "Settings", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("combobox", { name: "Language", exact: true }).selectOption("zh-CN");
  await dialog.getByRole("combobox", { name: "主题", exact: true }).selectOption("dark");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(dialog.getByRole("combobox", { name: "语言", exact: true })).toHaveValue("zh-CN");
  await page.screenshot({ path: testInfo.outputPath("settings-chinese-dark.png"), animations: "disabled" });
  await page.keyboard.press("Escape");
  await page.reload();
  await expect(page.getByRole("textbox", { name: "请求 URL", exact: true })).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.locator(".app-shell > header").getByRole("button", { name: "设置", exact: true }).click();
  await expect(dialog.getByRole("combobox", { name: "语言", exact: true })).toHaveValue("zh-CN");
  await expect(dialog.getByRole("combobox", { name: "主题", exact: true })).toHaveValue("dark");
  await dialog.getByRole("combobox", { name: "主题", exact: true }).selectOption("system");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.locator(".app-shell > header").getByRole("button", { name: "设置", exact: true }).click();
  await expect(dialog.getByRole("combobox", { name: "主题", exact: true })).toHaveValue("system");
  await expectNoHorizontalOverflow(page);
});
