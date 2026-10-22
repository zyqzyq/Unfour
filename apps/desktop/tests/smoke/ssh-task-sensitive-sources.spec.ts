import { expect, test } from "@playwright/test";

// Isolated browser fixtures; no remote SSH execution or credential storage.
for (const sourceState of ["shadowed", "removed", "public"] as const) {
  test(`SSH recalled sensitive provenance stays locked when Workspace source is ${sourceState}`, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
    await page.route("**/browser-mocks/ssh-tasks.ts*", async (route) => {
      await route.fulfill({ contentType: "application/javascript", body: `
        import { mockStore } from "./state.ts";
        import { UNHANDLED } from "./types.ts";
        const workspaceId = "mock-workspace";
        mockStore.sshConnections = [{ id: "conn", workspaceId, name: "Local fixture", host: "127.0.0.1", port: 22, username: "fixture", authKind: "none", keyPath: null, credentialRef: null, deletedAt: null, revision: 1 }];
        const workspaceVariable = { id: "workspace-var", workspaceId, key: "REGION", value: "workspace-${sourceState === "public" ? "public" : "private"}", isEnabled: true, isSecret: ${sourceState !== "public"}, revision: 1 };
        mockStore.workspaceVariables = ${sourceState === "removed" ? "[]" : "[workspaceVariable]"};
        mockStore.workspaceEnvironments = [{ id: "env", workspaceId, name: "Public environment", isActive: true, deletedAt: null,
          variables: [{ ...workspaceVariable, id: "env-var", environmentId: "env", value: "environment-public", isSecret: false }] },
          { id: "private-env", workspaceId, name: "Private environment", isActive: false, deletedAt: null,
          variables: [{ ...workspaceVariable, id: "private-env-var", environmentId: "private-env", value: "environment-private", isSecret: true }] }];
        const task = { id: "task", workspaceId, name: "Sensitive source fixture", description: "", sortOrder: 0, createdAt: "now", updatedAt: "now", deletedAt: null };
        const steps = [{ id: "step", workspaceId, taskId: "task", name: "Deploy", stepType: "command", position: 0, enabled: true, configVersion: 1, configJson: { command: "echo {{REGION}} {{PUBLIC}}", timeoutSeconds: 10 }, createdAt: "now", updatedAt: "now", deletedAt: null }];
        const detail = { task, steps, localBinding: { taskId: "task", workspaceId, defaultConnectionId: "conn", lastUsedConnectionId: "conn" } };
        const run = { id: "past", workspaceId, taskId: "task", connectionId: "conn", status: "success", startedAt: "2026-10-21T00:00:00Z", finishedAt: "2026-10-21T00:00:01Z", logPath: "fixture.log", errorMessage: null,
          context: { version: 1, environmentId: "env", taskVersion: "old", environmentUpdatedAt: null, inputs: {
            REGION: { source: "workspace", secret: ${sourceState !== "shadowed"} }, PUBLIC: { source: "manual", value: "ordinary" } } } };
        window.__sshStarts = 0;
        export function handleSshTaskMock(command) {
          if (command === "ssh_tasks_list") return [task];
          if (command === "ssh_task_get") return detail;
          if (command === "ssh_task_runs_list") return [run];
          if (command === "ssh_task_run") { window.__sshStarts++; return run; }
          return UNHANDLED;
        }
      ` });
    });
    await page.goto("/");
    await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal" }).click();
    await page.getByRole("tab", { name: "Tasks", exact: true }).click();
    await page.getByRole("treeitem").filter({ hasText: "Sensitive source fixture" }).click();
    await page.getByRole("button", { name: "Run", exact: true }).last().click();
    const dialog = page.getByRole("dialog", { name: "Run SSH Task", exact: true });
    const region = () => dialog.getByLabel("REGION", { exact: true });
    const environment = dialog.getByRole("combobox", { name: "Variable environment", exact: true });
    await expect(region()).toHaveValue("environment-public");
    await expect(region()).toHaveAttribute("type", "text");
    await dialog.getByRole("button", { name: "Use previous parameters" }).click();
    await expect(region()).toHaveValue(sourceState === "removed" ? "" : `workspace-${sourceState === "public" ? "public" : "private"}`);
    await expect(region()).toHaveAttribute("type", "password");
    const lock = dialog.getByRole("button", { name: "REGION: Sensitive input (never saved)", exact: true });
    await expect(lock).toBeDisabled();
    await lock.focus();
    await page.keyboard.press("Space");
    await expect(lock).toHaveAttribute("aria-pressed", "true");
    await expect(region()).toHaveAttribute("type", "password");
    // A manual source edit must keep the secret provenance, including partial edits.
    await region().fill("edited-private");
    await expect(lock).toBeDisabled();
    await expect(region()).toHaveAttribute("type", "password");
    await dialog.getByRole("button", { name: "Show secret value", exact: true }).click();
    await expect(region()).toHaveAttribute("type", "text");
    await environment.selectOption("private-env");
    await expect(region()).toHaveValue("environment-private");
    await expect(region()).toHaveAttribute("type", "password");
    await expect(lock).toBeDisabled();
    await dialog.getByRole("button", { name: "Show secret value", exact: true }).click();
    await expect(region()).toHaveAttribute("type", "text");
    await environment.selectOption("env");
    await expect(region()).toHaveValue("environment-public");
    await expect(region()).toHaveAttribute("type", "text");
    await dialog.getByRole("button", { name: "REGION: Mark as sensitive", exact: true }).click();
    await expect(region()).toHaveAttribute("type", "password");
    await dialog.getByRole("button", { name: "REGION: Remove sensitive marking", exact: true }).click();
    await expect(region()).toHaveAttribute("type", "text");
    await environment.selectOption("private-env");
    await expect(region()).toHaveAttribute("type", "password");
    if (sourceState === "shadowed") await page.screenshot({ animations: "disabled", path: "test-results/ssh-task-sensitive-sources.png" });
    expect(await page.evaluate("window.__sshStarts")).toBe(0);
    expect(errors).toEqual([]);
  });
}
