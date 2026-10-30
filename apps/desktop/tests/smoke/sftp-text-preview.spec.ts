import { expect, test } from "@playwright/test";

// Browser fixtures only; this does not validate a real SSH/SFTP server or xterm.
for (const locale of ["en", "zh-CN"] as const) {
  test(`${locale}: SFTP text preview scrolls and closes without transfer`, async ({ page }, testInfo) => {
    const zh = locale === "zh-CN";
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.stack ?? error.message));
    await page.addInitScript(({ locale, theme }) => {
      localStorage.setItem("unfour.locale", locale);
      localStorage.setItem("unfour.theme", theme);
    }, { locale, theme: zh ? "dark" : "light" });
    // Keep this SFTP layout test independent of the PTY renderer. The full-page
    // run's xterm dimensions failure is recorded in the verification document.
    await page.route("**/components/TerminalPane.tsx*", async (route) => {
      await route.fulfill({ contentType: "application/javascript", body: `
        export function TerminalPane() { return "PTY fixture"; }
      ` });
    });
    await page.route("**/browser-mocks/ssh.ts*", async (route) => {
      const response = await route.fetch();
      await route.fulfill({ response, body: await response.text() + `
        mockStore.sshConnections = [{ id: "preview-conn", workspaceId: "mock-workspace", name: "Preview fixture", host: "fixture", port: 22, username: "demo", authKind: "none", keyPath: null, credentialRef: null, deletedAt: null, revision: 1 }];
      ` });
    });
    await page.route("**/browser-mocks/sftp.ts*", async (route) => {
      await route.fulfill({ contentType: "application/javascript", body: `
        import { UNHANDLED } from "./types.ts";
        window.__previewCalls = [];
        export function handleSftpMock(command, args) {
          if (!command.startsWith("ssh_sftp_")) return UNHANDLED;
          window.__previewCalls.push(command);
          const input = args?.input ?? {};
          if (command === "ssh_sftp_open") return { ...input, connectionId: "preview-conn", homePath: "/" };
          if (command === "ssh_sftp_transfers_list") return [];
          if (command === "ssh_sftp_list_directory") return { ...input, connectionId: "preview-conn", entries: [{ name: "application.log", path: "/application.log", kind: "file", size: 8192, modifiedAt: null }] };
          if (command === "ssh_sftp_preview_text") return { content: Array.from({ length: 400 }, (_, i) => "Line " + i + " — UTF-8 文本 " + "x".repeat(180)).join("\\n"), size: 81920 };
          throw new Error("Unexpected fixture operation");
        }
      ` });
    });
    await page.goto("/");
    await page.getByRole("navigation", { name: zh ? "模块" : "Modules" })
      .getByRole("button", { name: zh ? "SSH 终端" : "SSH Terminal" }).click();
    await page.getByRole("treeitem").filter({ hasText: "Preview fixture" }).dblclick();
    await page.getByRole("button", { name: zh ? "展开远程文件面板" : "Open Remote Files panel", exact: true }).click();
    const row = page.getByRole("row", { name: /application\.log/ });
    await row.dblclick();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await row.click({ button: "right" });
    await page.getByRole("menuitem", { name: zh ? "预览" : "Preview", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: /application\.log/ });
    const content = dialog.getByLabel(zh ? "远程文本内容" : "Remote text content");
    await expect(content).toContainText("Line 399");
    expect(await content.evaluate((element) => element.scrollHeight > element.clientHeight && element.scrollWidth > element.clientWidth)).toBe(true);
    await expect(dialog.getByRole("button", { name: zh ? "复制内容" : "Copy content" })).toBeEnabled();
    await page.screenshot({ path: testInfo.outputPath(`preview-${locale}.png`), animations: "disabled" });
    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    const calls = await page.evaluate(() => (window as unknown as { __previewCalls: string[] }).__previewCalls);
    expect(calls.filter((command) => command === "ssh_sftp_preview_text")).toHaveLength(1);
    expect(calls.some((command) => /download|upload|delete|rename|create/.test(command))).toBe(false);
    expect(errors).toEqual([]);
  });
}
