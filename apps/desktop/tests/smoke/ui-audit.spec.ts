import { expect, test } from "@playwright/test";

for (const locale of ["en", "zh-CN"] as const) {
  for (const theme of ["light", "dark"] as const) {
    test(`${locale} ${theme}: discover Flow and MCP without losing the API draft`, async ({ page }, testInfo) => {
      const zh = locale === "zh-CN";
      await page.addInitScript(({ locale, theme }) => {
        localStorage.setItem("unfour.locale", locale);
        localStorage.setItem("unfour.theme", theme);
      }, { locale, theme });
      await page.setViewportSize({ width: 960, height: 600 });
      await page.goto("/");
      const url = page.getByRole("textbox", { name: zh ? "请求 URL" : "Request URL", exact: true });
      await url.fill("https://example.invalid/retained");
      const mcp = page.getByRole("button", { name: zh ? "MCP 设置" : "MCP settings", exact: true });
      await expect(mcp).toBeInViewport({ ratio: 1 });
      await mcp.click();
      const dialog = page.getByRole("dialog");
      await expect(dialog.getByRole("button", { name: "MCP", exact: true })).toHaveAttribute("aria-current", "page");
      const refresh = dialog.getByRole("button", { name: zh ? "刷新状态" : "Refresh status" });
      await expect(refresh).toBeEnabled();
      await refresh.click();
      await expect(refresh).toBeEnabled();
      await page.screenshot({ path: testInfo.outputPath("mcp.png"), animations: "disabled" });
      await page.keyboard.press("Escape");
      await expect(url).toHaveValue("https://example.invalid/retained");
      await page.keyboard.press("Control+Shift+P");
      const search = page.getByRole("combobox");
      await search.fill("Flow");
      await expect(page.getByRole("option")).toHaveCount(1);
      await search.press("Enter");
      await expect(page.getByRole("heading", { name: zh ? "创建第一个 Flow" : "Create your first Flow" })).toBeVisible();
      await page.screenshot({ path: testInfo.outputPath("flow-start.png"), animations: "disabled" });
      await page.getByRole("button", { name: zh ? "创建 Flow" : "Create Flow", exact: true }).click();
      await expect(page.getByRole("textbox", { name: zh ? "Flow 名称" : "Flow name" })).toHaveValue(zh ? "新建 Flow" : "New Flow");
      await page.keyboard.press("Control+Shift+P");
      await search.fill("MCP");
      await search.press("Enter");
      await expect(dialog.getByRole("button", { name: "MCP", exact: true })).toHaveAttribute("aria-current", "page");
      await expect.poll(() => dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true);
      await page.keyboard.press("Escape");
      await page.getByRole("navigation").getByRole("button", { name: zh ? "API 客户端" : "API Client", exact: true }).click();
      await expect(url).toHaveValue("https://example.invalid/retained");
      expect(await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)).toBe(false);
    });
  }
}

test("MCP failure stays local and refresh recovers without configuring a client", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/mcp.ts*", async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const body = source.replace(
      'if (command === "mcp_binary_path") {',
      'if (command === "mcp_binary_path") { if (!localStorage.getItem("audit.mcp.recovered")) throw new Error("Fixture unavailable");',
    );
    expect(body).not.toBe(source);
    await route.fulfill({ response, body });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "MCP settings", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("Refresh status to retry");
  await page.screenshot({ path: testInfo.outputPath("mcp-error.png"), animations: "disabled" });
  await page.evaluate(() => localStorage.setItem("audit.mcp.recovered", "true"));
  await page.getByRole("button", { name: "Refresh status" }).click();
  await expect(page.getByText("MCP binary not found")).toBeVisible();
  await expect(page.getByText("Some MCP status checks failed.", { exact: false })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Configure Codex" })).toBeDisabled();
});
