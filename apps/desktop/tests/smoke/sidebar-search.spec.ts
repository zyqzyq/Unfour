import { expect, test } from "@playwright/test";

for (const locale of ["en", "zh-CN"] as const) {
  test(`${locale}: compact local sidebar filters preserve loaded trees and terminal panes`, async ({ page }, testInfo) => {
    const text = locale === "en" ? {
      database: "Database", ssh: "SSH Terminal", connect: "Connect", collapse: "Collapse", expand: "Expand",
      databaseSearch: "Search database resources", sshSearch: "Search SSH connections", clear: "Clear search",
      noDatabase: "No matching database resources", noSsh: "No matching SSH connections",
    } : {
      database: "数据库", ssh: "SSH 终端", connect: "连接", collapse: "Collapse", expand: "Expand",
      databaseSearch: "搜索数据库资源", sshSearch: "搜索 SSH 连接", clear: "清空搜索",
      noDatabase: "没有匹配的数据库资源", noSsh: "没有匹配的 SSH 连接",
    };
    await page.addInitScript((locale) => {
      localStorage.setItem("unfour.locale", locale);
      localStorage.setItem("unfour.theme", locale === "en" ? "light" : "dark");
    }, locale);
    // Disposable browser fixtures only; no native DB or SSH server is accessed.
    await page.route("**/browser-mocks/state.ts*", async (route) => {
      const response = await route.fetch();
      await route.fulfill({ response, body: await response.text() + `
        mockStore.databaseConnections.push({
          id: 'search-db', workspaceId: 'mock-workspace', name: 'Search database',
          driver: 'postgres', database: 'app', host: 'example.test', port: 5432,
          username: 'fixture', sslMode: null, sqlitePath: null, credentialRef: null,
          readOnly: true, createdAt: '2026-01-01', updatedAt: '2026-01-01',
          deletedAt: null, revision: 1, syncStatus: 'local', remoteId: null
        });
        mockStore.sshConnections.push({
          id: 'search-ssh', workspaceId: 'mock-workspace', name: 'Search terminal',
          host: 'example.test', port: 22, username: 'fixture', authKind: 'none',
          keyPath: null, credentialRef: null, createdAt: '2026-01-01', updatedAt: '2026-01-01',
          deletedAt: null, revision: 1, syncStatus: 'local', remoteId: null
        });
        mockStore.sshSessions.push({
          sessionId: 'search-session', workspaceId: 'mock-workspace', connectionId: 'search-ssh',
          host: 'example.test', username: 'fixture', authKind: 'none', reconnectAttempt: 0,
          status: 'connected', cols: 120, rows: 32, createdAt: '2026-01-01', updatedAt: '2026-01-01'
        });
      ` });
    });
    await page.setViewportSize({ width: 960, height: 600 });
    await page.goto("/");
    const nav = page.getByRole("navigation");
    await nav.getByRole("button", { name: text.database, exact: true }).click();
    const sidebar = page.getByRole("complementary");
    const databaseSearch = sidebar.getByRole("textbox", { name: text.databaseSearch });
    await expect(databaseSearch).toBeVisible();
    expect((await databaseSearch.boundingBox())?.height).toBe(28);
    await sidebar.getByRole("button", { name: "Search database", exact: true }).click({ button: "right" });
    await page.getByRole("menuitem", { name: text.connect, exact: true }).click();
    const users = sidebar.getByRole("button", { name: "users", exact: true });
    await expect(users).toBeVisible();
    const catalog = sidebar.locator('[data-tree-id="search-db:catalog:app"]');
    await catalog.getByRole("button", { name: text.collapse, exact: true }).click();
    await databaseSearch.fill("UsErS");
    await expect(users).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "public", exact: true })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "reporting", exact: true })).toHaveCount(0);
    await page.screenshot({ path: testInfo.outputPath("database-search.png") });
    await databaseSearch.fill("no-result");
    await expect(sidebar.getByText(text.noDatabase, { exact: true })).toBeVisible();
    await sidebar.getByRole("button", { name: text.clear, exact: true }).click();
    await expect(databaseSearch).toBeFocused();
    await expect(catalog).toHaveAttribute("aria-expanded", "false");
    await catalog.getByRole("button", { name: text.expand, exact: true }).click();
    await expect(users).toBeVisible();

    await nav.getByRole("button", { name: text.ssh, exact: true }).click();
    const terminal = page.locator(".xterm-screen").first();
    await expect(terminal).toBeVisible();
    const terminalElement = await terminal.elementHandle();
    const sshSearch = sidebar.getByRole("textbox", { name: text.sshSearch });
    expect((await sshSearch.boundingBox())?.height).toBe(28);
    await sshSearch.fill("EXAMPLE.TEST");
    await expect(sidebar.getByRole("button", { name: "Search terminal", exact: true })).toBeVisible();
    await sshSearch.fill("no-result");
    await expect(sidebar.getByText(text.noSsh, { exact: true })).toBeVisible();
    await expect(terminal).toBeVisible();
    expect(await terminalElement?.evaluate((element) => element.isConnected)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath("ssh-no-results.png") });
    await sidebar.getByRole("button", { name: text.clear, exact: true }).click();
    await expect(sidebar.getByRole("button", { name: "Search terminal", exact: true })).toBeVisible();
  });
}
