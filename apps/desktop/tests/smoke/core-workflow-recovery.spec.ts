import { expect, test } from "@playwright/test";
import { prepareWorkbenchScene } from "./workbench-fixtures";

// All executions below use disposable browser command-bus fixtures. No native
// database, SSH connection, production workspace or external HTTP is accessed.
test("API failure stays actionable on Tests and retry sends the edited draft", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("https://ue.example.test/**", (route) => route.request().url().endsWith("/failed")
    ? route.abort("failed")
    : route.fulfill({ contentType: "application/json", body: '{"ok":true}' }));
  await page.goto("/");
  const url = page.getByRole("textbox", { name: "Request URL", exact: true });
  await url.fill("https://ue.example.test/ok");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect(page.getByText("200 OK", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: /^Tests/ }).click();
  await url.fill("https://ue.example.test/failed");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect(page.getByRole("button", { name: "Send", exact: true }).first()).toBeEnabled();
  await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("api-failure.png"), animations: "disabled" });
  await url.fill("https://ue.example.test/repaired");
  const retry = page.waitForRequest("https://ue.example.test/repaired");
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await retry;
  await expect(page.getByText("200 OK", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
});

test("Database Messages retains transport failure details and editor can retry", async ({ page }, testInfo) => {
  await prepareWorkbenchScene(page, "light");
  // Override only the script command. Existing connection/schema fixtures stay local.
  await page.route("**/browser-mocks/database.ts*", async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const body = source.replace('["database_script_execute", unsupportedScript]',
      '["database_script_execute", (args) => { if (args.input.sql.includes("ue_failure")) throw { code: "NETWORK_ERROR", message: "UE fixture connection interrupted" }; return { statements: [{ index: 1, start: 0, end: args.input.sql.length, sql: args.input.sql, status: "success", result: executeMockDatabaseQuery(args.input), error: null }], stopped: false }; }]');
    expect(body).not.toBe(source);
    await route.fulfill({ response, body });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Database", exact: true }).click();
  const tree = page.getByRole("complementary").getByRole("tree");
  await tree.getByRole("button", { name: "Atlas PostgreSQL", exact: true }).click({ button: "right" });
  await page.getByRole("menuitem", { name: "Connect", exact: true }).click();
  await tree.locator('[data-tree-id="workbench-db:catalog:app:saved-sql"]').getByRole("button", { name: "Expand", exact: true }).click();
  await tree.getByRole("button", { name: "Active members", exact: true }).dblclick();
  const editor = page.locator(".monaco-editor:visible");
  await editor.click({ position: { x: 100, y: 30 } });
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText("SELECT 'ue_failure';");
  await page.getByRole("button", { name: "Run All", exact: true }).click();
  const resultPanel = page.getByRole("tab", { name: "Messages", exact: true }).locator("xpath=ancestor::section[1]");
  await expect(resultPanel.getByText("UE fixture connection interrupted", { exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Messages", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Messages", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(resultPanel.getByText("UE fixture connection interrupted", { exact: true })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath("database-messages.png"), animations: "disabled" });
  await editor.click({ position: { x: 100, y: 30 } });
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText("SELECT 1;");
  await page.getByRole("button", { name: "Run All", exact: true }).click();
  await expect(resultPanel.getByText("UE fixture connection interrupted", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("cell", { name: "mock-workspace", exact: true })).toBeVisible();
  const nav = page.getByRole("navigation", { name: "Modules" });
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await nav.getByRole("button", { name: "Database", exact: true }).click();
  await expect(editor).toContainText("SELECT 1;");
  await expect(page.getByRole("cell", { name: "mock-workspace", exact: true })).toBeVisible();
});

test("SSH task output preserves a scrolled reading position until following latest", async ({ page }, testInfo) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/tauri/ssh.ts*", async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const body = source.replace(/function registerSshTaskRunChannel\(onMessage\) \{/, (match) =>
      `${match}\nwindow.__ueTaskEvent = onMessage;`);
    expect(body).not.toBe(source);
    await route.fulfill({ response, body });
  });
  await page.route("**/browser-mocks/ssh-tasks.ts*", (route) => route.fulfill({ contentType: "application/javascript", body: `
    import { mockStore } from "./state.ts";
    import { UNHANDLED } from "./types.ts";
    const workspaceId = "mock-workspace";
    mockStore.sshConnections = [{ id: "conn", workspaceId, name: "UE test connection", host: "127.0.0.1", port: 22, username: "fixture", authKind: "none", deletedAt: null, revision: 1 }];
    const task = { id: "task", workspaceId, name: "UE log test", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null };
    const step = { id: "step", workspaceId, taskId: "task", name: "Read output", stepType: "command", position: 0, enabled: true, configVersion: 1, configJson: { command: "echo fixture", timeoutSeconds: 10 }, createdAt: "", updatedAt: "", deletedAt: null };
    const run = { id: "ue-run", workspaceId, taskId: "task", connectionId: "conn", status: "running", startedAt: "2026-11-09T00:00:00Z", finishedAt: null, logPath: "", errorMessage: null };
    export function handleSshTaskMock(command) {
      if (command === "ssh_tasks_list") return [task];
      if (command === "ssh_task_get") return { task, steps: [step], localBinding: { taskId: "task", workspaceId, defaultConnectionId: "conn", lastUsedConnectionId: "conn" } };
      if (command === "ssh_task_runs_list") return [run];
      if (command === "ssh_task_run") return run;
      return UNHANDLED;
    }
  ` }));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal" }).click();
  await page.getByRole("tab", { name: "Tasks", exact: true }).click();
  await page.getByRole("treeitem").filter({ hasText: "UE log test" }).click();
  await page.getByRole("button", { name: "Run", exact: true }).last().click();
  await page.getByRole("dialog").getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const output = page.locator("pre").filter({ hasText: "line 0" });
  const emit = (data: string) => page.evaluate((text) => {
    const emitEvent = (window as unknown as { __ueTaskEvent: (event: unknown) => void }).__ueTaskEvent;
    emitEvent({ runId: "ue-run", taskId: "task", kind: "output", stepId: "step", stream: "stdout", data: text, createdAt: new Date().toISOString() });
  }, data);
  await emit(Array.from({ length: 100 }, (_, i) => `line ${i}\n`).join(""));
  await expect(output).toContainText("line 99");
  await output.evaluate((node) => { node.scrollTop = 0; node.dispatchEvent(new Event("scroll")); });
  await emit("new diagnostic\n");
  await expect(output).toContainText("new diagnostic");
  await expect.poll(() => output.evaluate((node) => node.scrollTop)).toBe(0);
  await page.screenshot({ path: testInfo.outputPath("ssh-reading-position.png"), animations: "disabled" });
  const nav = page.getByRole("navigation", { name: "Modules" });
  await nav.getByRole("button", { name: "Database", exact: true }).click();
  await emit("received while hidden\n");
  await nav.getByRole("button", { name: "SSH Terminal", exact: true }).click();
  await expect(output).toContainText("received while hidden");
  await expect.poll(() => output.evaluate((node) => node.scrollTop)).toBe(0);
  await page.setViewportSize({ width: 960, height: 600 });
  await expect(page.getByRole("button", { name: "Follow latest output", exact: true })).toBeInViewport({ ratio: 1 });
  await page.getByRole("button", { name: "Follow latest output", exact: true }).click();
  await expect(output).toBeFocused();
  await expect.poll(() => output.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight)).toBeLessThan(2);
  await emit("following again\n");
  await expect(output).toContainText("following again");
  await expect.poll(() => output.evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight)).toBeLessThan(2);
});
