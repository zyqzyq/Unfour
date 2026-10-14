import { expect, test } from "@playwright/test";

for (const locale of ["en", "zh-CN"] as const) for (const theme of ["light", "dark"] as const) {
  test(`${locale} ${theme}: database first use and SSH clipboard recovery`, async ({ page }, testInfo) => {
    const zh = locale === "zh-CN";
    await page.addInitScript(({ locale, theme }) => {
      localStorage.setItem("unfour.locale", locale);
      localStorage.setItem("unfour.theme", theme);
      Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
        writeText: async (text: string) => {
          if (!localStorage.getItem("audit.clipboard.allowed")) throw new Error("Clipboard denied fixture");
          localStorage.setItem("audit.clipboard.content", text);
        },
      } });
    }, { locale, theme });
    // Inject only disposable log fixtures into this browser response. No SSH
    // connection or system clipboard is accessed by this test.
    await page.route("**/model/terminal-state.ts*", async (route) => {
      const response = await route.fetch();
      const source = await response.text();
      const fixture = JSON.stringify([{ sessionId: "fixture", kind: "output", data: "fixture ready\nauthorization: fixture-secret", createdAt: "2026-10-14" }]);
      const body = source.replaceAll("terminalEvents: [],", `terminalEvents: ${fixture},`);
      expect(body).not.toBe(source);
      await route.fulfill({ response, body });
    });
    await page.setViewportSize({ width: 960, height: 600 });
    await page.goto("/");
    const nav = page.getByRole("navigation");
    const database = nav.getByRole("button", { name: zh ? "数据库" : "Database", exact: true });
    await database.click();
    const create = page.getByRole("button", { name: zh ? "添加连接" : "Add connection", exact: true });
    await expect(create).toBeVisible();
    await expect(page.getByRole("button", { name: zh ? "全部运行" : "Run All", exact: true })).toHaveCount(0);
    await page.screenshot({ path: testInfo.outputPath("database-start.png"), animations: "disabled" });
    await database.click(); // Collapse sidebar; primary creation stays reachable.
    await expect(create).toBeInViewport({ ratio: 1 });
    await create.click();
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: zh ? "取消" : "Cancel", exact: true }).click();
    await expect(create).toBeFocused();
    await nav.getByRole("button", { name: zh ? "SSH 终端" : "SSH Terminal", exact: true }).click();
    await page.getByRole("button", { name: zh ? "切换底部面板" : "Toggle bottom panel", exact: true }).click();
    await expect(page.getByText(zh ? "连接事件" : "Connection Events", { exact: true })).toBeVisible();
    const copy = page.getByRole("button", { name: zh ? "复制终端日志" : "Copy terminal logs", exact: true });
    await expect(copy).toBeEnabled();
    await copy.click();
    await expect(page.getByRole("alert")).toContainText(zh ? "剪贴板权限" : "clipboard access");
    await page.screenshot({ path: testInfo.outputPath("clipboard-error.png"), animations: "disabled" });
    await page.evaluate(() => localStorage.setItem("audit.clipboard.allowed", "true"));
    await copy.click();
    await expect(page.getByRole("status")).toContainText(zh ? "日志已复制" : "Logs copied");
    const content = await page.evaluate(() => localStorage.getItem("audit.clipboard.content"));
    expect(content).toContain("fixture ready");
    expect(content).toContain("<redacted>");
    expect(content).not.toContain("fixture-secret");
    await expect(page.getByRole("alert")).toHaveCount(0);
    await page.screenshot({ path: testInfo.outputPath("clipboard-recovered.png"), animations: "disabled" });
    await page.getByRole("button", { name: zh ? "收起底部面板" : "Collapse bottom panel", exact: true }).click();
    await expect(copy).toHaveCount(0);
  });
}
