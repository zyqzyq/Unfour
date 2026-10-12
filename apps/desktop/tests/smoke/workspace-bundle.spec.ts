import { expect, test } from "@playwright/test";

for (const locale of ["en", "zh-CN"]) {
  const notices = locale === "en" ? {
    create: "Create and Switch",
    auto: "PROD with Auto uses read-only MCP defaults. Write and execution actions are blocked.",
    guarded: "PROD with Guarded (guarded) overrides",
    full: "PROD with Full access (full_access) overrides",
    policyContext: "Saving MCP permissions uses the saved environment type.",
    savedProd: "Saved environment: PROD",
    savedAutoProd: "Auto currently resolves to Read-only (read_only) using the saved environment type.",
  } : {
    create: "创建并切换",
    auto: "PROD + Auto 的 MCP 默认只读，写入和执行类操作会被拦截。",
    guarded: "PROD + 受保护 (guarded) 会覆盖",
    full: "PROD + 完全访问 (full_access) 会覆盖",
    policyContext: "保存 MCP 权限时沿用已保存的环境类型。",
    savedProd: "已保存环境：PROD",
    savedAutoProd: "Auto 按已保存的环境类型实际生效为：只读 (read_only)。",
  };
  test(`${locale}: Workspace bundle preview stays readable at a compact viewport`, async ({ page }, testInfo) => {
    await page.addInitScript((language) => localStorage.setItem("unfour.locale", language), locale);
    await page.setViewportSize({ width: 960, height: 600 });
    const preview = {
      content: '{"format":"unfour-workspace","version":1}',
      preview: {
        name: "Portable workspace (Copy 1)",
        environmentType: "prod",
        counts: { variables: 4, environments: 2, environmentVariables: 8, collections: 3, folders: 6, requests: 12, connections: 4, sshTasks: 3, sshSteps: 7, savedSql: 4, flows: 2 },
        reconfigure: Array.from({ length: 8 }, (_, index) => ({ entityId: `connection-${index}`, name: `Deployment host ${index + 1}`, code: "connection" })),
      },
    };
    // Browser smoke only mocks native file selection. Backend parsing, remapping and
    // commit/rollback are exercised with isolated SQLite databases in Rust tests.
    await page.route("**/packages/command-client/src/tauri/workspace.ts*", async (route) => {
      const response = await route.fetch();
      const source = await response.text();
      const body = source.replace('return call("workspace_bundle_pick");', `return Promise.resolve(${JSON.stringify(preview)});`);
      expect(body).not.toBe(source);
      await route.fulfill({ response, body });
    });
    await page.goto("/");
    await expect(page.getByRole("textbox", { name: locale === "en" ? "Request URL" : "请求 URL", exact: true })).toBeVisible();
    const workspaceMenu = page.locator("button.w-\\[220px\\]");
    await workspaceMenu.click();
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-menu-${locale}.png`) });
    await page.getByRole("menuitem", { name: locale === "en" ? "Import Workspace from File…" : "从文件导入 Workspace…", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("textbox")).toHaveValue(preview.preview.name);
    await expect(dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true })).toHaveValue("disabled");
    await expect(dialog.getByRole("status")).toHaveCount(0);
    await dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true }).selectOption("auto");
    await expect(dialog.getByText(locale === "en"
      ? "After import, Auto resolves to Read-only (read_only) using the file's PROD environment type."
      : "导入后，Auto 按文件中的 PROD 环境类型实际生效为：只读 (read_only)。", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("status")).toContainText(notices.auto);
    await expect(dialog.getByRole("button", { name: notices.create, exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-auto-preview-${locale}.png`) });
    await dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true }).selectOption("guarded");
    await expect(dialog.getByRole("status")).toContainText(notices.guarded);
    await expect(dialog.getByText("Deployment host 1", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("button", { name: notices.create, exact: true })).toBeInViewport({ ratio: 1 });
    await expect(dialog).toHaveCSS("opacity", "1");
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-preview-${locale}.png`) });
    await dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true }).selectOption("full_access");
    await expect(dialog.getByRole("status")).toContainText(notices.full);
    await expect(dialog.getByRole("button", { name: notices.create, exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-preview-full-${locale}.png`) });
    await dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true }).selectOption("read_only");
    await expect(dialog.getByRole("status")).toHaveCount(0);
    await dialog.getByRole("button", { name: locale === "en" ? "Cancel" : "取消", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await workspaceMenu.click();
    const activeName = (await workspaceMenu.getAttribute("title")) ?? "";
    expect(activeName).not.toBe("");
    await page.getByRole("menuitem", { name: locale === "en" ? "Export Current Workspace…" : "导出当前 Workspace…", exact: true }).click();
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveCSS("opacity", "1");
    await expect(dialog.getByText(activeName, { exact: true })).toBeVisible();
    await expect(dialog.getByRole("button", { name: locale === "en" ? "Choose Save Location…" : "选择保存位置…", exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-export-${locale}.png`) });
    await dialog.getByLabel(locale === "en" ? "Export mode" : "导出方式", { exact: true }).selectOption("backup");
    const save = dialog.getByRole("button", { name: locale === "en" ? "Choose Save Location…" : "选择保存位置…", exact: true });
    await expect(save).toBeDisabled();
    await dialog.getByLabel(locale === "en" ? "Backup password" : "备份密码", { exact: true }).fill("disposable-visual-password");
    await dialog.getByLabel(locale === "en" ? "Confirm backup password" : "确认备份密码", { exact: true }).fill("disposable-visual-password");
    await expect(save).toBeEnabled();
    await expect(save).toBeInViewport({ ratio: 1 });
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-backup-${locale}.png`) });
    await dialog.getByRole("button", { name: locale === "en" ? "Cancel" : "取消", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await page.keyboard.press("Control+Shift+P");
    const commands = page.getByRole("combobox", { name: locale === "en" ? "Search commands" : "搜索命令", exact: true });
    await commands.fill("Workspace");
    await page.getByRole("option", { name: locale === "en" ? "Workspace: Export Current Workspace…" : "Workspace：导出当前 Workspace…", exact: true }).click();
    await expect(dialog.getByText(activeName, { exact: true })).toBeVisible();
    await expect(dialog).toHaveCount(1);
    await dialog.getByRole("button", { name: locale === "en" ? "Cancel" : "取消", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await page.keyboard.press("Control+Shift+P");
    await commands.fill("Workspace");
    await page.getByRole("option", { name: locale === "en" ? "Workspace: Import from File…" : "Workspace：从文件导入…", exact: true }).click();
    await expect(dialog.getByRole("textbox")).toHaveValue(preview.preview.name);
    await expect(dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true })).toHaveValue("disabled");
  });

  test(`${locale}: imported MCP permissions can be changed independently in security settings`, async ({ page }, testInfo) => {
    await page.addInitScript((language) => localStorage.setItem("unfour.locale", language), locale);
    await page.setViewportSize({ width: 960, height: 600 });
    const file = { content: "browser-only-fixture", preview: { name: "Imported project", environmentType: "prod", counts: {}, reconfigure: [] } };
    // Native import is covered by Rust tests. Here seed the browser store with the
    // selected policy so the actual switcher, cache and update commands are exercised.
    await page.route("**/packages/command-client/src/tauri/workspace.ts*", async (route) => {
      const response = await route.fetch();
      const source = await response.text();
      let body = source.replace('return call("workspace_bundle_pick");', `return Promise.resolve(${JSON.stringify(file)});`);
      body = body.replace('return call("workspace_bundle_import", { content, name, options });', 'return createWorkspace(name, "prod", options.mcpPolicy ?? "disabled");');
      expect(body).not.toBe(source);
      await route.fulfill({ response, body });
    });
    await page.goto("/");
    const workspaceMenu = page.locator("button.w-\\[220px\\]");
    await workspaceMenu.click();
    await page.getByRole("menuitem", { name: locale === "en" ? "Import Workspace from File…" : "从文件导入 Workspace…", exact: true }).click();
    let dialog = page.getByRole("dialog");
    await expect(dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true })).toHaveValue("disabled");
    await dialog.getByRole("button", { name: locale === "en" ? "Create and Switch" : "创建并切换", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(workspaceMenu).toHaveAttribute("title", "Imported project");
    await workspaceMenu.click();
    await expect(page.getByText(locale === "en" ? "MCP: Disabled" : "MCP：已禁用", { exact: true })).toBeVisible();
    await page.getByRole("menuitem", { name: locale === "en" ? "Workspace security settings" : "工作区安全设置", exact: true }).click();
    dialog = page.getByRole("dialog");
    const policy = dialog.getByLabel(locale === "en" ? "MCP permissions" : "MCP 权限", { exact: true });
    const environment = dialog.getByLabel(locale === "en" ? "Workspace environment type" : "工作区环境类型", { exact: true });
    await expect(policy).toHaveValue("disabled");
    await expect(environment).toHaveValue("prod");
    await environment.selectOption("dev");
    await expect(dialog.getByText(notices.savedProd, { exact: true })).toBeVisible();
    await policy.selectOption("auto");
    await expect(dialog.getByRole("status")).toContainText(notices.auto);
    await expect(dialog.getByRole("status")).toContainText(notices.policyContext);
    await policy.selectOption("guarded");
    await expect(dialog.getByRole("status")).toContainText(notices.guarded);
    await policy.selectOption("full_access");
    await expect(dialog.getByRole("status")).toContainText(notices.full);
    await expect(dialog.getByRole("button", { name: locale === "en" ? "Close" : "关闭", exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-prod-warning-${locale}.png`) });
    await dialog.getByRole("button", { name: locale === "en" ? "Save MCP permissions" : "保存 MCP 权限", exact: true }).click();
    await expect(dialog.getByText(locale === "en" ? "Saved permissions: Full access (full_access)" : "已保存权限：完全访问 (full_access)", { exact: true })).toBeVisible();
    await expect(environment).toHaveValue("dev");
    await dialog.getByRole("button", { name: locale === "en" ? "Close" : "关闭", exact: true }).click();
    await workspaceMenu.click();
    await expect(page.getByText("PROD", { exact: true }).last()).toBeVisible();
    await expect(page.getByText(locale === "en" ? "MCP: Full access" : "MCP：完全访问", { exact: true })).toBeVisible();
    await page.getByRole("menuitem", { name: locale === "en" ? "Workspace security settings" : "工作区安全设置", exact: true }).click();
    await expect(environment).toHaveValue("prod");
    await environment.selectOption("test");
    await policy.selectOption("auto");
    await expect(dialog.getByText(notices.savedAutoProd, { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: locale === "en" ? "Save environment" : "保存环境类型", exact: true }).click();
    await expect(dialog.getByText(locale === "en" ? "Auto currently resolves to Guarded (guarded) using the saved environment type." : "Auto 按已保存的环境类型实际生效为：受保护 (guarded)。", { exact: true })).toBeVisible();
    await expect(dialog.getByText(locale === "en" ? "Saved permissions: Full access (full_access)" : "已保存权限：完全访问 (full_access)", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: locale === "en" ? "Save MCP permissions" : "保存 MCP 权限", exact: true }).click();
    await expect(dialog.getByText(locale === "en" ? "Saved permissions: Auto (auto) → Guarded (guarded)" : "已保存权限：自动 (auto) → 受保护 (guarded)", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("button", { name: locale === "en" ? "Close" : "关闭", exact: true })).toBeInViewport({ ratio: 1 });
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-security-${locale}.png`) });
  });
}
