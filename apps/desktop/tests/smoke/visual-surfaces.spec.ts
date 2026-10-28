import { expect, test, type Locator } from "@playwright/test";

async function background(locator: Locator) {
  return locator.evaluate((element) => getComputedStyle(element).backgroundColor);
}

async function expectReadable(locator: Locator) {
  await expect(locator).toBeVisible();
  const contrast = await locator.evaluate((element) => {
    const luminance = (color: string) => {
      const channels = (color.match(/[\d.]+/g) ?? []).slice(0, 3).map((n) => {
        const c = Number(n) / 255;
        return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      });
      return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
    };
    let parent: Element | null = element;
    while (parent && getComputedStyle(parent).backgroundColor === "rgba(0, 0, 0, 0)") parent = parent.parentElement;
    if (!parent) throw new Error("No opaque surface found");
    const a = luminance(getComputedStyle(element).color);
    const b = luminance(getComputedStyle(parent).backgroundColor);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });
  expect(contrast).toBeGreaterThanOrEqual(4.5);
}

// Browser-only fixtures: no native database, SSH server, or HTTP request is used.
for (const theme of ["light", "dark"] as const) {
  test(`${theme}: data canvases and workbench surfaces`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.addInitScript((mode) => {
      localStorage.setItem("unfour.locale", "en");
      localStorage.setItem("unfour.theme", mode);
    }, theme);
    await page.route("https://example.test/visual", (route) => route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({ workspace: "Visual fixture", records: [{ id: 1, name: "Unfour", enabled: true }], total: 1 }),
    }));
    await page.route("**/browser-mocks/state.ts*", async (route) => {
      const response = await route.fetch();
      await route.fulfill({ response, body: await response.text() + `
        mockStore.databaseConnections.push({
          id: 'visual-db', workspaceId: 'mock-workspace', name: 'Visual database',
          driver: 'postgres', database: 'app', host: 'example.test', port: 5432,
          username: 'fixture', sslMode: null, sqlitePath: null, credentialRef: null,
          readOnly: true, createdAt: '2026-01-01', updatedAt: '2026-01-01',
          deletedAt: null, revision: 1, syncStatus: 'local', remoteId: null
        });
        mockStore.savedSql.push({
          id: 'visual-query', workspaceId: 'mock-workspace', connectionId: 'visual-db',
          catalog: 'app', schema: 'public', name: 'Visual query',
          sql: '-- Inspect workspace records\\nSELECT id, name FROM users LIMIT 100;',
          createdAt: '2026-01-01', updatedAt: '2026-01-01'
        });
        mockStore.sshConnections.push({
          id: 'visual-ssh', workspaceId: 'mock-workspace', name: 'Visual terminal',
          host: 'example.test', port: 22, username: 'fixture', authKind: 'agent',
          keyPath: null, credentialRef: null, createdAt: '2026-01-01', updatedAt: '2026-01-01',
          deletedAt: null, revision: 1, syncStatus: 'local', remoteId: null
        });
        mockStore.sshSessions.push({
          sessionId: 'visual-session', workspaceId: 'mock-workspace', connectionId: 'visual-ssh',
          host: 'example.test', port: 22, username: 'fixture', authKind: 'agent',
          status: 'connected', cols: 120, rows: 32, createdAt: '2026-01-01', updatedAt: '2026-01-01'
        });
        mockStore.sshEvents.push({ sessionId: 'visual-session', kind: 'output',
          data: 'Visual fixture terminal\\r\\n$ ls\\r\\nREADME.md  packages  crates\\r\\n$ ', createdAt: '2026-01-01' });
      ` });
    });
    const screenshot = async (name: string) => {
      await page.screenshot({ path: testInfo.outputPath(`${name}-${theme}.png`), animations: "disabled" });
    };
    await page.goto("/");
    await page.getByRole("textbox", { name: "Request URL", exact: true }).fill("https://example.test/visual");
    await page.getByRole("button", { name: "Send", exact: true }).first().click();
    await expect(page.locator(".monaco-editor").first()).toBeVisible();
    const canvas = theme === "light" ? "rgb(255, 255, 255)" : "rgb(30, 30, 46)";
    await expect(page.locator(".monaco-editor:visible")).toHaveCSS("background-color", canvas);
    await expectReadable(page.getByText("Collections", { exact: true }));
    const chrome = await background(page.locator(".app-shell > header"));
    expect(chrome).not.toBe(await background(page.getByRole("complementary").last()));
    await expect(page.locator(".u-statusbar")).toHaveCSS("background-color", chrome);
    await screenshot("api");

    const nav = page.getByRole("navigation");
    await nav.getByRole("button", { name: "Database", exact: true }).click();
    const tree = page.getByRole("complementary").getByRole("tree");
    await tree.getByRole("button", { name: "Visual database", exact: true }).click({ button: "right" });
    await page.getByRole("menuitem", { name: "Connect", exact: true }).click();
    await tree.getByRole("button", { name: "users", exact: true }).dblclick();
    await expect(page.getByRole("cell", { name: "mock-db", exact: true })).toBeVisible();
    const grid = page.getByRole("table").locator("..");
    await expect(grid).toHaveCSS("background-color", canvas);
    await expectReadable(page.getByRole("columnheader", { name: "id TEXT" }).locator("span").last());
    const activeTableTab = page.getByRole("tab", { name: "users", exact: true });
    const tabSurface = await background(activeTableTab.locator(".."));
    const nextToolbar = page.getByRole("button", { name: "Structure", exact: true }).locator("../..");
    await expect(nextToolbar).toHaveCSS("background-color", tabSurface);
    await screenshot("database-table");
    const cell = page.getByRole("cell", { name: "mock-db", exact: true });
    await cell.click();
    const selected = await background(cell);
    await cell.hover();
    expect(await background(cell)).toBe(selected);
    expect(selected).not.toBe("rgba(0, 0, 0, 0)");
    await page.getByRole("button", { name: "Close users", exact: true }).focus();
    await page.keyboard.press("Shift+Tab");
    await expect(activeTableTab).toBeFocused();
    await expect(activeTableTab).toHaveCSS("outline-style", "solid");
    await screenshot("database-focus-selected");
    await tree.locator('[data-tree-id="visual-db:catalog:app:saved-sql"]').getByRole("button", { name: "Expand", exact: true }).click();
    await tree.getByRole("button", { name: "Visual query", exact: true }).dblclick();
    await expect(page.locator(".monaco-editor:visible")).toBeVisible();
    await expect(page.locator(".monaco-editor:visible")).toHaveCSS("background-color", canvas);
    await screenshot("database-sql");

    await nav.getByRole("button", { name: "SSH Terminal", exact: true }).click();
    await expect(page.locator(".xterm-screen").first()).toBeVisible();
    const terminalTab = page.getByRole("tab", { name: /fixture@example.test/ });
    await expect(terminalTab.locator("..")).toHaveCSS("background-color", "rgb(30, 30, 46)");
    await expectReadable(terminalTab);
    await expectReadable(page.getByRole("button", { name: "Close fixture@example.test", exact: true }));
    await screenshot("ssh");
    expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);

    // Both editors remain mounted across module changes; verify live theme switching too.
    await nav.getByRole("button", { name: "API Client", exact: true }).click();
    await page.locator(".app-shell > header").getByRole("button", { name: "Settings", exact: true }).click();
    await page.getByRole("dialog").getByLabel("Theme", { exact: true }).selectOption(theme === "light" ? "dark" : "light");
    await page.keyboard.press("Escape");
    const switchedCanvas = theme === "light" ? "rgb(30, 30, 46)" : "rgb(255, 255, 255)";
    await expect(page.locator(".monaco-editor:visible")).toHaveCSS("background-color", switchedCanvas);
    await nav.getByRole("button", { name: "Database", exact: true }).click();
    await expect(page.locator(".monaco-editor:visible")).toHaveCSS("background-color", switchedCanvas);
  });
}
