import { expect, test } from "@playwright/test";

test("test workspace/environment selection targets the next request and module switches retain drafts", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockWorkspace.name = "UE Test A";
      mockWorkspace.environmentType = "test";
      mockState.workspaces.push({ ...mockWorkspace, id: "ue-test-b", name: "UE Test B", isDefault: false });
      mockStore.workspaceEnvironments = [
        ["mock-workspace", "env-a1", "Test A1", true],
        ["mock-workspace", "env-a2", "Test A2", false],
        ["ue-test-b", "env-b", "Test B", true]
      ].map(([workspaceId, id, name, isActive]) => ({ id, workspaceId, name, isActive, sortOrder: 0, revision: 1, deletedAt: null, createdAt: "", updatedAt: "",
        variables: [{ id: id + "-url", workspaceId, environmentId: id, key: "base_url", value: "https://ue.example.test/" + id, isEnabled: true, isSecret: false, deletedAt: null }] }));
    ` });
  });
  const requested: string[] = [];
  await page.route("https://ue.example.test/**", (route) => {
    requested.push(route.request().url());
    return route.fulfill({ contentType: "application/json", body: '{"test":true}' });
  });
  await page.goto("/");
  const url = page.getByRole("textbox", { name: "Request URL", exact: true });
  const environment = page.getByRole("button", { name: "Active variable environment" });
  const nav = page.getByRole("navigation", { name: "Modules" });
  await expect(environment).toHaveText("Test A1");
  await url.fill("{{base_url}}/health");
  await environment.click();
  await page.getByRole("menuitem", { name: "Test A2", exact: true }).click();
  await expect(environment).toHaveText("Test A2");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://ue.example.test/env-a2/health"]);
  await expect(page.getByText("200 OK", { exact: true })).toBeVisible();

  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await page.getByRole("textbox", { name: "Flow name", exact: true }).fill("Unsaved UE Flow");
  await nav.getByRole("button", { name: "Database", exact: true }).click();
  await nav.getByRole("button", { name: "SSH Terminal", exact: true }).click();
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Flow name", exact: true })).toHaveValue("Unsaved UE Flow");
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await expect(url).toHaveValue("{{base_url}}/health");
  await expect(page.getByText("200 OK", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: /UE Test A/ }).click();
  await page.getByRole("menuitem", { name: /UE Test B/ }).click();
  await page.getByRole("dialog", { name: "Leave workspace?" }).getByRole("button", { name: "Discard and leave" }).click();
  await expect(environment).toHaveText("Test B");
  await expect(url).not.toHaveValue("{{base_url}}/health");
  await url.fill("{{base_url}}/health");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://ue.example.test/env-a2/health", "https://ue.example.test/env-b/health"]);
  await expect(page.getByText("200 OK", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: /UE Test B/ }).click();
  await page.getByRole("menuitem", { name: /UE Test A/ }).click();
  await expect(environment).toHaveText("Test A2");
  // Workspace layout restoration may select its last persisted module.
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await expect(url).toHaveValue("{{base_url}}/health");
  await expect(page.getByText("200 OK", { exact: true })).toBeVisible();
});

test("existing Flow failure recovery edits, saves and runs the new revision", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/flow.ts*", (route) => route.fulfill({ contentType: "application/javascript", body: `
    import { UNHANDLED } from "./types.ts";
    let definition = { id: "ue-flow", workspaceId: "mock-workspace", name: "UE recovery", revision: 1, inputs: [],
      steps: [{ id: "wait", name: "Wait fixture", kind: "wait", durationMs: 10, timeoutMs: 1000, next: null }] };
    let run;
    export function handleFlowMock(command, args) {
      if (command === "flow_list") return [definition];
      if (command === "flow_save") { definition = { ...args.input, revision: definition.revision + 1 }; return definition; }
      if (command === "flow_runs_list") return run ? [run] : [];
      if (command === "flow_run_get") return run;
      if (command === "flow_run") {
        const status = definition.steps[0].durationMs === 20 ? "succeeded" : "failed";
        run = { id: "ue-run-r" + definition.revision, workspaceId: definition.workspaceId, flowId: definition.id,
          definition: structuredClone(definition), context: args.input, resources: {}, status, error: null,
          startedAt: "2026-11-09T00:00:00Z", finishedAt: "2026-11-09T00:00:01Z",
          steps: [{ stepId: "wait", status, durationMs: 10, error: status === "failed" ? "UE synthetic failure" : null, attempts: [], output: null }] };
        return run;
      }
      return UNHANDLED;
    }
  ` }));
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "UE recovery", exact: true }).click();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Run", exact: true }).click();
  await page.getByRole("button", { name: "Locate: Wait fixture · Failed", exact: true }).click();
  await expect(page.getByRole("complementary", { name: "Run detail" }).getByRole("alert")).toHaveText("UE synthetic failure");
  await page.getByRole("button", { name: "Edit this step", exact: true }).click();
  await page.getByLabel("Wait (ms)", { exact: true }).fill("20");
  await expect(page.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByText("r2", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Run", exact: true }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByRole("complementary", { name: "Run detail" }).getByText("Succeeded", { exact: true })).toBeVisible();
  await expect(page.getByText("ue-run-r2", { exact: true })).toBeVisible();
});
