import { expect, test } from "@playwright/test";

test("database tree connects, lazily browses catalogs, previews tables, and confirms saved SQL deletion", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  // Browser-only fixtures exercise command dispatch and the real page/controller
  // wiring without accessing a native database or credentials.
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockStore.databaseConnections.push({
        id: 'tree-server', workspaceId: 'mock-workspace', name: 'Tree fixture',
        driver: 'postgres', database: 'app', host: 'example.test', port: 5432,
        username: 'fixture', sslMode: null, sqlitePath: null, credentialRef: null,
        readOnly: true, createdAt: '2026-01-01', updatedAt: '2026-01-01',
        deletedAt: null, revision: 1, syncStatus: 'local', remoteId: null
      });
      mockStore.savedSql.push({
        id: 'tree-query', workspaceId: 'mock-workspace', connectionId: 'tree-server',
        catalog: 'app', schema: 'public', name: 'Fixture query', sql: 'select 1',
        createdAt: '2026-01-01', updatedAt: '2026-01-01'
      });
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation").getByRole("button", { name: "Database", exact: true }).click();
  const tree = page.getByRole("complementary").getByRole("tree");
  const connection = tree.getByRole("button", { name: "Tree fixture", exact: true });
  await expect(tree.getByRole("button", { name: "users", exact: true })).toHaveCount(0);
  await connection.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Connect", exact: true }).click();
  const users = tree.getByRole("button", { name: "users", exact: true });
  await expect(users).toBeVisible();
  await users.dblclick();
  await expect(page.getByRole("cell", { name: "mock-db", exact: true })).toBeVisible();

  const reporting = tree.locator('[data-tree-id="tree-server:catalog:reporting"]');
  await reporting.getByRole("button", { name: "Expand", exact: true }).click();
  await expect(tree.getByRole("button", { name: "metrics", exact: true })).toBeVisible();
  const views = tree.locator('[data-tree-id="tree-server:catalog:reporting:schema:metrics:views"]');
  await views.getByRole("button", { name: "Expand", exact: true }).click();
  await expect(tree.getByRole("button", { name: "daily_revenue", exact: true })).toBeVisible();
  await reporting.getByRole("button", { name: "Collapse", exact: true }).click();
  await expect(tree.getByRole("button", { name: "daily_revenue", exact: true })).toHaveCount(0);
  await expect(users).toBeVisible();

  const savedGroup = tree.locator('[data-tree-id="tree-server:catalog:app:saved-sql"]');
  await savedGroup.getByRole("button", { name: "Expand", exact: true }).click();
  const saved = tree.getByRole("button", { name: "Fixture query", exact: true });
  await saved.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(saved).toBeVisible();
  await saved.click({ button: "right" });
  await page.getByRole("menuitem", { name: "Delete", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Delete", exact: true }).click();
  await expect(saved).toHaveCount(0);
  await page.screenshot({ path: testInfo.outputPath("database-tree.png") });
});
