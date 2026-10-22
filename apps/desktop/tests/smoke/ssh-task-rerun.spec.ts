import { expect, test } from "@playwright/test";

// UI fixture only. Rust persistence/execution is covered by native loopback tests.
test("SSH history and previous parameters open the same editable run dialog", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/ssh-tasks.ts*", async (route) => {
    await route.fulfill({ contentType: "application/javascript", body: `
      import { mockStore } from "./state.ts";
      import { UNHANDLED } from "./types.ts";
      const workspaceId = "mock-workspace";
      mockStore.sshConnections = [{ id: "conn", workspaceId, name: "Local fixture", host: "127.0.0.1", port: 22, username: "fixture", authKind: "none", keyPath: null, credentialRef: null, deletedAt: null, revision: 1 }];
      mockStore.workspaceVariables = [{ id: "var", workspaceId, key: "VERSION", value: "current-default", isEnabled: true, isSecret: false }];
      const task = { id: "task", workspaceId, name: "Deploy fixture", description: "", sortOrder: 0, createdAt: "now", updatedAt: "now", deletedAt: null };
      const steps = [{ id: "step", workspaceId, taskId: "task", name: "Deploy", stepType: "command", position: 0, enabled: true, configVersion: 1, configJson: { command: "echo {{VERSION}} {{CUSTOM}}", timeoutSeconds: 10 }, createdAt: "now", updatedAt: "now", deletedAt: null }];
      const detail = { task, steps, localBinding: { taskId: "task", workspaceId, defaultConnectionId: "conn", lastUsedConnectionId: "conn" } };
      const run = { id: "failed", workspaceId, taskId: "task", connectionId: "deleted-conn", status: "failed", startedAt: "2026-10-21T00:00:00Z", finishedAt: "2026-10-21T00:00:01Z", logPath: "fixture.log", errorMessage: "fixture failure",
        context: { version: 1, environmentId: null, taskVersion: "old", environmentUpdatedAt: null, inputs: { VERSION: { source: "manual", value: "previous-version" }, CUSTOM: { source: "manual" } } } };
      window.__sshStarts = 0;
      export function handleSshTaskMock(command) {
        if (command === "ssh_tasks_list") return [task];
        if (command === "ssh_task_get") return detail;
        if (command === "ssh_task_runs_list") return [run];
        if (command === "ssh_task_run_log_read") return "previous log";
        if (command === "ssh_task_run") { window.__sshStarts++; return { ...run, id: "new", status: "success" }; }
        return UNHANDLED;
      }
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal" }).click();
  await page.getByRole("tab", { name: "Tasks", exact: true }).click();
  await page.getByRole("treeitem").filter({ hasText: "Deploy fixture" }).click();
  await page.getByRole("button", { name: "Run", exact: true }).last().click();
  const dialog = page.getByRole("dialog", { name: "Run SSH Task", exact: true });
  await expect(dialog.getByLabel("VERSION", { exact: true })).toHaveValue("current-default");
  await dialog.getByRole("button", { name: "Use previous parameters" }).click();
  await expect(dialog.getByLabel("VERSION", { exact: true })).toHaveValue("previous-version");
  await expect(dialog.getByLabel("CUSTOM", { exact: true })).toHaveAttribute("type", "password");
  await expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  await dialog.getByRole("combobox", { name: "SSH connection", exact: true }).selectOption("conn");
  await dialog.getByLabel("CUSTOM", { exact: true }).fill("ephemeral");
  await expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled();
  await page.screenshot({ animations: "disabled", path: "test-results/ssh-task-rerun-dialog.png" });
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.getByRole("radio", { name: "Run History", exact: true }).click();
  await page.getByRole("button", { name: "Run again", exact: true }).click();
  await expect(dialog.getByLabel("VERSION", { exact: true })).toHaveValue("previous-version");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.locator("tbody tr").filter({ hasText: "fixture failure" }).click();
  await expect(page.getByText("previous log", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Run again", exact: true }).last().click();
  await expect(dialog).toBeVisible();
  expect(await page.evaluate("window.__sshStarts")).toBe(0);
  expect(errors).toEqual([]);
});
