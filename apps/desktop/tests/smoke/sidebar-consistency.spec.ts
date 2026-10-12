import { expect, test, type Locator } from "@playwright/test";

async function expectCompactCreate(button: Locator) {
  await expect(button).toBeVisible();
  await expect(button.locator("svg.lucide-plus")).toHaveCount(1);
  await expect(button).toHaveAttribute("title", await button.getAttribute("aria-label") ?? "");
  const bounds = await button.boundingBox();
  expect(bounds?.height).toBe(28);
  expect(bounds?.width).toBe(28);
}

for (const locale of ["en", "zh-CN"] as const) {
  test(`${locale}: resource creation uses compact header actions`, async ({ page }, testInfo) => {
    const labels = locale === "en" ? {
      ssh: "SSH Terminal", database: "Database", collection: "New collection",
      import: "Import collection", connection: "New Connection", databaseConnection: "New database connection",
      environment: "Active variable environment", manage: "Manage variables", newEnvironment: "New variable environment",
      flow: "New Flow", flowName: "Flow name", empty: "No Flows yet. Use + to create one.",
    } : {
      ssh: "SSH 终端", database: "数据库", collection: "新建集合",
      import: "导入集合", connection: "新建连接", databaseConnection: "新建数据库连接",
      environment: "当前变量环境", manage: "管理变量", newEnvironment: "新建变量环境",
      flow: "新建 Flow", flowName: "Flow 名称", empty: "还没有 Flow，点击 + 新建。",
    };
    await page.addInitScript(({ locale, theme }) => {
      localStorage.setItem("unfour.locale", locale);
      localStorage.setItem("unfour.theme", theme);
    }, { locale, theme: locale === "en" ? "light" : "dark" });
    await page.setViewportSize({ width: 960, height: 600 });
    await page.goto("/");
    await expectCompactCreate(page.getByRole("button", { name: labels.collection, exact: true }));
    const importButton = page.getByRole("button", { name: labels.import, exact: true });
    await expect(importButton).toHaveAttribute("title", labels.import);
    expect((await importButton.boundingBox())?.height).toBe(28);
    await expect(page.getByRole("button", { name: locale === "en" ? "New Request" : "新建请求", exact: true }).locator("svg.lucide-plus")).toHaveCount(1);
    const modules = page.getByRole("navigation");
    await modules.getByRole("button", { name: labels.ssh, exact: true }).click();
    await expectCompactCreate(page.getByRole("complementary").getByRole("button", { name: labels.connection, exact: true }));
    await modules.getByRole("button", { name: labels.database, exact: true }).click();
    await expectCompactCreate(page.getByRole("button", { name: labels.databaseConnection, exact: true }));
    await page.getByRole("button", { name: labels.environment, exact: true }).click();
    await page.getByRole("menuitem", { name: labels.manage, exact: true }).click();
    await expectCompactCreate(page.getByRole("button", { name: labels.newEnvironment, exact: true }));
    await page.screenshot({ path: testInfo.outputPath("environment-sidebar.png") });
    await modules.getByRole("button", { name: "Flow", exact: true }).click();
    const sidebar = page.getByRole("region", { name: "Flow", exact: true });
    await expect(sidebar.getByText(labels.empty, { exact: true })).toBeVisible();
    const create = sidebar.getByRole("button", { name: labels.flow, exact: true });
    await expectCompactCreate(create);
    await create.click();
    await expect(page.getByRole("textbox", { name: labels.flowName, exact: true })).toHaveValue(labels.flow);
    await expect(page.locator(".flow-canvas")).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("flow-sidebar.png") });
  });
}

test("Flow and SSH keep creation visible while long lists scroll", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  // Disposable browser definitions only. No native or external execution.
  const longName = "Release validation with a very long name that must fit a compact sidebar";
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      for (let index = 0; index < 50; index += 1) {
        mockStore.sshConnections.push({ id: 'sidebar-ssh-' + index, workspaceId: 'mock-workspace', name: 'SSH ' + index, host: 'example.test', port: 22, username: 'fixture' });
      }
    ` });
  });
  await page.route("**/browser-mocks/flow.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      for (let index = 0; index < 50; index += 1) {
        definitions.set('sidebar-flow-' + index, {
          id: 'sidebar-flow-' + index, workspaceId: 'mock-workspace',
          name: index === 0 ? ${JSON.stringify(longName)} : 'Flow ' + index,
          revision: 1, inputs: [], steps: [{id: 'wait', name: 'Wait', kind: 'wait', durationMs: 10, timeoutMs: 1000, next: null}]
        });
      }
    ` });
  });
  await page.setViewportSize({ width: 960, height: 600 });
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "SSH Terminal", exact: true }).click();
  const sshSidebar = page.getByRole("complementary");
  const sshCreate = sshSidebar.getByRole("button", { name: "New Connection", exact: true });
  const sshRows = sshSidebar.getByRole("treeitem");
  await expect(sshRows).toHaveCount(50);
  const sshHeaderBefore = await sshCreate.boundingBox();
  await sshRows.first().focus();
  await page.keyboard.press("End");
  await expect(sshRows.last()).toBeInViewport();
  await expect(sshCreate).toBeInViewport();
  expect((await sshCreate.boundingBox())?.y).toBe(sshHeaderBefore?.y);
  await page.screenshot({ path: testInfo.outputPath("ssh-list-scroll.png") });
  await page.getByRole("navigation").getByRole("button", { name: "Flow", exact: true }).click();
  const sidebar = page.getByRole("region", { name: "Flow", exact: true });
  const rows = sidebar.getByRole("treeitem");
  await expect(rows).toHaveCount(50);
  await rows.first().focus();
  await page.keyboard.press("ArrowDown");
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("textbox", { name: "Flow name", exact: true })).toHaveValue("Flow 1");
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("textbox", { name: "Flow name", exact: true })).toHaveValue(longName);
  await expect(rows.first()).toHaveAttribute("title", longName);
  const label = rows.first().getByRole("button", { name: longName, exact: true });
  expect(await label.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);
  const create = sidebar.getByRole("button", { name: "New Flow", exact: true });
  const headerBefore = await create.boundingBox();
  await page.keyboard.press("End");
  await expect(rows.last()).toBeInViewport();
  await expect(create).toBeInViewport();
  expect((await create.boundingBox())?.y).toBe(headerBefore?.y);
  await page.screenshot({ path: testInfo.outputPath("flow-list-scroll.png") });
});
