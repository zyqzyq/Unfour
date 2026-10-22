import { expect, test } from "@playwright/test";

// Keep this regression independent of sensitive-parameter presentation controls.
for (const entry of ["editor", "sidebar"] as const) {
  test(`SSH first recall restores parameters from ${entry} without visiting history`, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
    await page.route("**/browser-mocks/ssh-tasks.ts*", async (route) => {
      await route.fulfill({ contentType: "application/javascript", body: `
        import { mockStore } from "./state.ts";
        import { UNHANDLED } from "./types.ts";
        const workspaceId = "mock-workspace";
        mockStore.sshConnections = [{ id: "conn", workspaceId, name: "Local fixture", host: "127.0.0.1", port: 22, username: "fixture", authKind: "none", keyPath: null, credentialRef: null, deletedAt: null, revision: 1 }];
        mockStore.workspaceVariables = [];
        mockStore.workspaceEnvironments = [{ id: "saved-env", workspaceId, name: "Previous environment", isActive: false, deletedAt: null,
          variables: [{ id: "env-var", workspaceId, environmentId: "saved-env", key: "REGION", value: "resolved-region", isEnabled: true, isSecret: false }] }];
        const task = { id: "task", workspaceId, name: "Recall fixture", description: "", sortOrder: 0, createdAt: "now", updatedAt: "now", deletedAt: null };
        const steps = [{ id: "step", workspaceId, taskId: "task", name: "Deploy", stepType: "command", position: 0, enabled: true, configVersion: 1, configJson: { command: "echo {{VERSION}} {{REGION}}", timeoutSeconds: 10 }, createdAt: "now", updatedAt: "now", deletedAt: null }];
        const detail = { task, steps, localBinding: { taskId: "task", workspaceId, defaultConnectionId: "conn", lastUsedConnectionId: "conn" } };
        const run = { id: "past", workspaceId, taskId: "task", connectionId: "conn", status: "success", startedAt: "2026-10-21T00:00:00Z", finishedAt: "2026-10-21T00:00:01Z", logPath: "fixture.log", errorMessage: null,
          context: { version: 1, environmentId: "saved-env", taskVersion: "old", environmentUpdatedAt: null, inputs: { VERSION: { source: "manual", value: "previous-version" }, REGION: { source: "environment" } } } };
        window.__sshStarts = 0;
        window.__sshRunListReads = 0;
        let waitForHistory = true;
        const historyReaders = [];
        window.__sshReleaseHistory = () => { waitForHistory = false; historyReaders.splice(0).forEach((resolve) => resolve([run])); };
        export function handleSshTaskMock(command) {
          if (command === "ssh_tasks_list") return [task];
          if (command === "ssh_task_get") return detail;
          if (command === "ssh_task_runs_list") { window.__sshRunListReads++; return waitForHistory ? new Promise((resolve) => historyReaders.push(resolve)) : [run]; }
          if (command === "ssh_task_run") { window.__sshStarts++; return run; }
          return UNHANDLED;
        }
      ` });
    });
    await page.goto("/");
    await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal" }).click();
    await page.getByRole("tab", { name: "Tasks", exact: true }).click();
    const taskItem = page.getByRole("treeitem").filter({ hasText: "Recall fixture" });
    if (entry === "editor") await taskItem.getByRole("button", { name: "Recall fixture", exact: true }).click();
    else expect(await page.evaluate("window.__sshRunListReads")).toBe(0);
    await (entry === "sidebar" ? taskItem : page).getByRole("button", { name: "Run", exact: true }).last().click();

    const dialog = page.getByRole("dialog", { name: "Run SSH Task", exact: true });
    const environment = dialog.getByRole("combobox", { name: "Variable environment", exact: true });
    await expect(environment).toHaveValue("");
    await expect(dialog.getByLabel("VERSION", { exact: true })).toHaveValue("");
    await expect(dialog.getByLabel("REGION", { exact: true })).toHaveValue("");
    const recall = dialog.getByRole("button", { name: "Use previous parameters" });
    await recall.click();
    await expect(recall).toBeDisabled();
    await expect(dialog.getByRole("button", { name: "Starting...", exact: true })).toBeDisabled();
    await page.evaluate("window.__sshReleaseHistory()");
    await expect(dialog.getByLabel("VERSION", { exact: true })).toHaveValue("previous-version");
    await expect(environment).toHaveValue("saved-env");
    await expect(dialog.getByLabel("REGION", { exact: true })).toHaveValue("resolved-region");
    await expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeEnabled();
    if (entry === "sidebar") expect(await page.evaluate("window.__sshRunListReads")).toBe(1);
    expect(await page.evaluate("window.__sshStarts")).toBe(0);
    expect(errors).toEqual([]);
  });
}
