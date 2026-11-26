import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { prepareWorkbenchScene } from "./workbench-fixtures";

test("searches older SQL and confirms clearing all workspace history even when filtered", async ({ page }) => {
  await prepareWorkbenchScene(page, "light");
  await page.route("**/browser-mocks/database.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      const record = handlers.get("database_query_history_record");
      const entry = { workspaceId: "mock-workspace", connectionId: "workbench-db", connectionName: "Archived reporting DB",
        id: "old", sql: "SELECT 'rate_%';", status: "success", rowCount: 1, executedAt: "2020-01-02T12:00:00Z" };
      record({ input: entry });
      for (let i = 0; i < 25; i++) record({ input: { ...entry, id: "recent-" + i, sql: "SELECT " + i,
        executedAt: new Date().toISOString(), connectionName: "Atlas PostgreSQL" } });
      const clear = handlers.get("database_query_history_clear");
      let firstClear = true;
      handlers.set("database_query_history_clear", (args) => {
        if (firstClear) { firstClear = false; throw new Error("History storage is busy"); }
        return clear(args);
      });
      handlers.set("database_script_execute", () => { sessionStorage.setItem("history-executed", "yes"); });
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Database", exact: true }).click();
  await page.getByRole("button", { name: "New Query", exact: true }).click();
  const historyTab = page.getByRole("tab", { name: /History/ });
  await historyTab.click();
  const search = page.getByRole("searchbox", { name: "Search SQL history" });
  const archived = page.getByRole("button", { name: /Archived reporting DB/ });
  await expect(page.getByRole("button", { name: /SELECT 0.*Atlas PostgreSQL/ })).toBeVisible();
  await expect(archived).toHaveCount(0);
  await search.fill("RATE_%");
  await expect(archived).toBeVisible();
  const oldDate = await page.evaluate(() =>
    new Date("2020-01-02T12:00:00Z").toLocaleString("en", { dateStyle: "short", timeStyle: "medium" }));
  await expect(archived.locator("time")).toHaveText(oldDate);
  await archived.click();
  await expect(page.locator(".monaco-editor:visible")).toContainText("rate_%");
  expect(await page.evaluate(() => sessionStorage.getItem("history-executed"))).toBeNull();

  await historyTab.click();
  await expect(search).toHaveValue("RATE_%");
  await expect(archived).toBeVisible();
  const clear = page.getByRole("button", { name: "Clear History", exact: true });
  await clear.click();
  const dialog = page.getByRole("dialog", { name: "Clear workspace SQL history?" });
  await expect(dialog).toContainText("every connection in “Atlas Development”");
  await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(dialog).toBeHidden();
  await expect(archived).toBeVisible();

  await clear.click();
  await dialog.getByRole("button", { name: "Clear History" }).click();
  await expect(dialog.getByRole("alert")).toContainText("History storage is busy");
  await dialog.getByRole("button", { name: "Retry" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText("No SQL history matches your search.")).toBeVisible();
  await search.fill("");
  await expect(page.getByText("Executed SQL history will appear here.")).toBeVisible();
  await expect(clear).toBeDisabled();
});

test("browses and exports the returned rows after one SQL execution", async ({ page }) => {
  await prepareWorkbenchScene(page, "light");
  await page.addInitScript(() => {
    document.addEventListener("copy", (event) => {
      const text = event.defaultPrevented
        ? event.clipboardData?.getData("text/plain")
        : window.getSelection()?.toString();
      sessionStorage.setItem("copied", text ?? "");
      event.preventDefault();
    });
  });
  const rows = Array.from({ length: 800 }, (_, index) => [
    String(index + 1), index % 2 ? "matched" : "other",
  ]);
  await page.route("**/browser-mocks/database.ts*", async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const result = {
      columns: [{ name: "id", dataType: "integer" }, { name: "label", dataType: "text" }],
      rows, rowLimit: 1000, truncated: false, durationMs: 2, affectedRows: 0,
      safety: { classification: "read", requiresConfirmation: false, confirmed: true, message: null },
    };
    await route.fulfill({ response, body: source + `
      handlers.set("database_script_execute", ({ input }) => ({
        statements: [{ index: 1, start: 0, end: input.sql.length, sql: input.sql, status: "success",
          result: ${JSON.stringify(result)}, error: null }], stopped: false
      }));
      for (const command of ["database_script_execute", "database_query_execute", "database_table_browse"]) {
        const original = handlers.get(command);
        handlers.set(command, (args) => {
          sessionStorage.setItem("result-query-count", String(Number(sessionStorage.getItem("result-query-count")) + 1));
          return original(args);
        });
      }
    ` });
  });

  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Database", exact: true }).click();
  const tree = page.getByRole("complementary").getByRole("tree");
  await tree.getByRole("button", { name: "Atlas PostgreSQL", exact: true }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Connect", exact: true }).click();
  await tree.locator('[data-tree-id="workbench-db:catalog:app:saved-sql"]').getByRole("button", { name: "Expand" }).click();
  await tree.getByRole("button", { name: "Active members", exact: true }).dblclick();
  const editor = page.locator(".monaco-editor:visible");
  await editor.click({ position: { x: 100, y: 30 } });
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText("SELECT id, label FROM users ORDER BY id;");
  await page.getByLabel("Result row limit").selectOption("1000");
  await page.getByRole("button", { name: "Run All", exact: true }).click();

  const panel = page.getByRole("tab", { name: "Results", exact: true }).locator("xpath=ancestor::section[1]");
  const next = panel.getByRole("button", { name: "Next page", exact: true });
  const lastRow = panel.getByRole("cell", { name: "800", exact: true });
  await expect(next).toBeEnabled();
  const scrollArea = panel.getByRole("table").locator("..");
  await scrollArea.evaluate((element) => { element.scrollTop = element.scrollHeight; });
  expect(await scrollArea.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  await next.click();
  await expect(panel.getByText("101-200 of 800")).toBeVisible();
  await expect.poll(() => scrollArea.evaluate((element) => element.scrollTop)).toBe(0);
  await panel.getByRole("tab", { name: "Messages", exact: true }).click();
  await panel.getByRole("tab", { name: "Results", exact: true }).click();
  await expect(panel.getByText("101-200 of 800")).toBeVisible();
  while (!(await lastRow.count()) && await next.isEnabled()) {
    await next.click();
  }
  await expect(lastRow).toBeVisible();
  await lastRow.getByRole("button").click();
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Control+c");
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem("copied"))).toBe("800\tmatched");

  const label = lastRow.locator("..").getByRole("button", { name: "matched", exact: true });
  await label.click();
  await label.evaluate((cell) => {
    const text = cell.firstChild!;
    const range = document.createRange();
    range.setStart(text, 1);
    range.setEnd(text, 4);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
  });
  await page.keyboard.press("Control+c");
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem("copied"))).toBe("atc");
  await lastRow.getByRole("button").click({ modifiers: ["Shift"] });
  await page.keyboard.press("Control+c");
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem("copied"))).toBe("800\tmatched");

  await panel.getByRole("textbox", { name: "Filter rows" }).fill("matched");
  await expect.poll(() => scrollArea.evaluate((element) => element.scrollTop)).toBe(0);
  const scope = panel.getByRole("combobox", { name: "Copy and export scope" });
  await expect(scope.locator("option:checked")).toContainText("400");
  await panel.getByRole("button", { name: "Export", exact: true }).click();
  const downloaded = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Export JSON" }).click();
  const file = await (await downloaded).path();
  if (!file) throw new Error("The query result download did not produce a file");
  expect(JSON.parse(await readFile(file, "utf8"))).toEqual(
    rows.filter(([, label]) => label === "matched").map(([id, label]) => ({ id, label })),
  );
  await expect.poll(() => page.evaluate(() => sessionStorage.getItem("result-query-count"))).toBe("1");
});
