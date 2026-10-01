import { expect, test } from "@playwright/test";

for (const locale of ["en", "zh-CN"]) {
  test(`${locale}: Workspace bundle preview stays readable at a compact viewport`, async ({ page }, testInfo) => {
    await page.addInitScript((language) => localStorage.setItem("unfour.locale", language), locale);
    await page.setViewportSize({ width: 960, height: 600 });
    const preview = {
      content: '{"format":"unfour-workspace","version":1}',
      preview: {
        name: "Portable workspace (Copy 1)",
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
    await page.getByRole("menuitem", { name: locale === "en" ? "Import Workspace…" : "导入 Workspace…", exact: true }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("textbox")).toHaveValue(preview.preview.name);
    await expect(dialog.getByText("Deployment host 1", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("button", { name: locale === "en" ? "Create new Workspace" : "创建新 Workspace", exact: true })).toBeInViewport({ ratio: 1 });
    await expect(dialog).toHaveCSS("opacity", "1");
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-preview-${locale}.png`) });
    await dialog.getByRole("button", { name: locale === "en" ? "Cancel" : "取消", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await workspaceMenu.click();
    await page.getByRole("menuitem", { name: locale === "en" ? "Export Workspace…" : "导出 Workspace…", exact: true }).click();
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveCSS("opacity", "1");
    await page.screenshot({ animations: "disabled", path: testInfo.outputPath(`workspace-export-${locale}.png`) });
  });
}
