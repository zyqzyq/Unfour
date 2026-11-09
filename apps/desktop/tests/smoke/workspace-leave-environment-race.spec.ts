import { expect, test, type Page } from "@playwright/test";

async function fixture(page: Page) {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockWorkspace.name = "Test A"; mockWorkspace.environmentType = "test";
      mockState.workspaces.push({ ...mockWorkspace, id: "test-b", name: "Test B", isDefault: false });
      mockStore.workspaceEnvironments = [
        ["mock-workspace", "a1", true], ["mock-workspace", "a2", false],
        ["mock-workspace", "a3", false], ["test-b", "b1", true]
      ].map(([workspaceId, id, isActive]) => ({ id, workspaceId, name: id, isActive, revision: 1, sortOrder: 0, deletedAt: null, createdAt: "", updatedAt: "",
        variables: [{ id: id + "-url", workspaceId, environmentId: id, key: "base_url", value: "https://race.example.test/" + id, isEnabled: true, isSecret: false, deletedAt: null }] }));
    ` });
  });
  await page.route("**/browser-mocks/workspace.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: (await response.text()).replace("environments.setMockActiveEnvironment]", "delayedActivate]") + `
      window.environmentRace = { pending: [], calls: [] };
      async function delayedActivate(args) {
        window.environmentRace.calls.push(args.environmentId);
        await new Promise((resolve, reject) => window.environmentRace.pending.push({ id: args.environmentId, resolve, reject }));
        return environments.setMockActiveEnvironment(args);
      }
    ` });
  });
  const requested: string[] = [];
  await page.route("https://race.example.test/**", (route) => {
    requested.push(route.request().url());
    return route.fulfill({ contentType: "application/json", body: '{"isolated":true}' });
  });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("a1");
  return requested;
}
async function select(page: Page, name: string) {
  await page.getByRole("button", { name: "Active variable environment" }).click();
  await page.getByRole("menuitem", { name, exact: true }).click();
}
async function finish(page: Page, id: string, fail = false) {
  await page.evaluate(({ id, fail }) => {
    const state = (window as unknown as { environmentRace: { pending: { id: string; resolve: () => void; reject: (error: Error) => void }[] } }).environmentRace;
    const entry = state.pending.find((item) => item.id === id);
    if (!entry) throw new Error("Missing pending activation: " + id);
    state.pending = state.pending.filter((item) => item !== entry);
    if (fail) entry.reject(new Error("isolated activation failed")); else entry.resolve();
  }, { id, fail });
}
async function switchWorkspace(page: Page, from: string, to: string) {
  await page.getByRole("button", { name: new RegExp(from) }).click();
  await page.getByRole("menuitem", { name: new RegExp(to) }).click();
}

test("hidden unsaved Flow cancels or explicitly discards workspace leave", async ({ page }, testInfo) => {
  await fixture(page);
  const nav = page.getByRole("navigation", { name: "Modules" });
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await page.getByRole("textbox", { name: "Flow name", exact: true }).fill("Unsaved isolated Flow");
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await switchWorkspace(page, "Test A", "Test B");
  const dialog = page.getByRole("dialog", { name: "Leave workspace?" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
  await page.screenshot({ path: testInfo.outputPath("flow-leave.png"), animations: "disabled" });
  await page.keyboard.press("Enter");
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Flow name", exact: true })).toHaveValue("Unsaved isolated Flow");
  await switchWorkspace(page, "Test A", "Test B");
  await dialog.getByRole("button", { name: "Discard and leave" }).click();
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("b1");
});

test("create and delete workspace commands also require the dirty Flow leave decision", async ({ page }) => {
  await fixture(page);
  const nav = page.getByRole("navigation", { name: "Modules" });
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await page.getByRole("button", { name: /Test A/ }).click();
  await page.getByRole("menuitem", { name: "New workspace", exact: true }).click();
  const create = page.getByRole("dialog", { name: "New workspace", exact: true });
  await create.getByRole("textbox").fill("Created test workspace");
  await create.getByRole("button", { name: "Create", exact: true }).click();
  const leave = page.getByRole("dialog", { name: "Leave workspace?" });
  await leave.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(create.getByRole("textbox")).toHaveValue("Created test workspace");
  await create.getByRole("button", { name: "Create", exact: true }).click();
  await leave.getByRole("button", { name: "Discard and leave" }).click();
  await expect(page.getByRole("button", { name: /Created test workspace/ })).toBeVisible();
  await expect(create).toHaveCount(0);
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "Create Flow", exact: true }).click();
  await page.getByRole("button", { name: /Created test workspace/ }).click();
  await page.getByRole("menuitem", { name: "Delete current", exact: true }).click();
  const remove = page.getByRole("dialog", { name: "Delete workspace", exact: true });
  await remove.getByRole("button", { name: "Delete", exact: true }).click();
  await leave.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(remove).toBeVisible();
  await remove.getByRole("button", { name: "Delete", exact: true }).click();
  await leave.getByRole("button", { name: "Discard and leave" }).click();
  await expect(page.getByRole("button", { name: /Created test workspace/ })).toHaveCount(0);
});

test("variable editor retains its own leave protection for modules and workspaces", async ({ page }) => {
  await fixture(page);
  await page.getByRole("button", { name: "Active variable environment" }).click();
  await page.getByRole("menuitem", { name: "Manage variables", exact: true }).click();
  const name = page.getByRole("textbox", { name: "Name", exact: true });
  await name.fill("Unsaved environment name");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow", exact: true }).click();
  const leave = page.getByRole("dialog", { name: "Discard variable environment changes?" });
  await leave.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(name).toHaveValue("Unsaved environment name");
  await switchWorkspace(page, "Test A", "Test B");
  await leave.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(name).toHaveValue("Unsaved environment name");
  await switchWorkspace(page, "Test A", "Test B");
  await leave.getByRole("button", { name: "Discard changes", exact: true }).click();
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("b1");
});

for (const existing of [false, true]) test(`${existing ? "existing" : "new"} Flow retains invalid JSON when workspace leave is cancelled`, async ({ page }) => {
  if (existing) await page.route("**/browser-mocks/flow.ts*", (route) => route.fulfill({ contentType: "application/javascript", body: `
    import { UNHANDLED } from "./types.ts";
    export function handleFlowMock(command, args) {
      if (command === "flow_list") return args.workspaceId === "mock-workspace" ? [{ id: "saved", workspaceId: "mock-workspace", name: "Saved fixture", revision: 1,
        inputs: [{ name: "payload", type: "json", required: false, secret: false, default: {} }],
        steps: [{ id: "wait", name: "Wait fixture", kind: "wait", durationMs: 10, timeoutMs: 1000, next: null }] }] : [];
      if (command === "flow_runs_list") return [];
      return UNHANDLED;
    }
  ` }));
  await fixture(page);
  const nav = page.getByRole("navigation", { name: "Modules" });
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: existing ? "Saved fixture" : "New Flow", exact: true }).click();
  if (!existing) {
    await page.getByRole("button", { name: "Add input", exact: true }).click();
    await page.getByLabel("Input name", { exact: true }).fill("payload");
    await page.getByRole("combobox", { name: "Type", exact: true }).selectOption("json");
  }
  const raw = page.getByLabel("Default (optional)", { exact: true });
  await raw.fill('{"unfinished":');
  await expect(page.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await switchWorkspace(page, "Test A", "Test B");
  await page.getByRole("dialog", { name: "Leave workspace?" }).getByRole("button", { name: "Cancel", exact: true }).click();
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await expect(raw).toHaveValue('{"unfinished":');
  await switchWorkspace(page, "Test A", "Test B");
  await page.getByRole("dialog", { name: "Leave workspace?" }).getByRole("button", { name: "Discard and leave" }).click();
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("b1");
  await switchWorkspace(page, "Test B", "Test A");
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("a1");
  await nav.getByRole("button", { name: "API Client", exact: true }).click();
  await nav.getByRole("button", { name: "Flow", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Flow name", exact: true })).toHaveCount(0);
  if (existing) {
    await page.getByRole("button", { name: "Saved fixture", exact: true }).click();
    await expect(raw).toHaveValue("{}");
  }
});

test("delayed activation blocks immediate Send, failure keeps target and retry succeeds", async ({ page }) => {
  const requested = await fixture(page);
  await page.getByRole("textbox", { name: "Request URL", exact: true }).fill("{{base_url}}/health");
  await select(page, "a2");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect(page.getByText("Environment switching. Wait for it to finish before sending.", { exact: true }).first()).toBeVisible();
  expect(requested).toEqual([]);
  await finish(page, "a2", true);
  await expect(page.getByText(/isolated activation failed/).first()).toBeVisible();
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("a1");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://race.example.test/a1/health"]);
  await select(page, "a2"); await finish(page, "a2");
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("a2");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://race.example.test/a1/health", "https://race.example.test/a2/health"]);
});

test("rapid choices serialize and old workspace callbacks stay isolated", async ({ page }) => {
  const requested = await fixture(page);
  await select(page, "a2"); await select(page, "a3");
  const calls = () => page.evaluate(() => (window as unknown as { environmentRace: { calls: string[] } }).environmentRace.calls);
  await expect.poll(calls).toEqual(["a2"]);
  await finish(page, "a2");
  await expect.poll(calls).toEqual(["a2", "a3"]);
  await switchWorkspace(page, "Test A", "Test B");
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("b1");
  await finish(page, "a3");
  await page.getByRole("textbox", { name: "Request URL", exact: true }).fill("{{base_url}}/health");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://race.example.test/b1/health"]);
  await switchWorkspace(page, "Test B", "Test A");
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("a3");
  await page.getByRole("textbox", { name: "Request URL", exact: true }).fill("{{base_url}}/health");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://race.example.test/b1/health", "https://race.example.test/a3/health"]);
});

test("attempted reverse completions cannot undo the last selected environment", async ({ page }) => {
  const requested = await fixture(page);
  await select(page, "a2"); await select(page, "a3");
  const calls = () => page.evaluate(() => (window as unknown as { environmentRace: { calls: string[] } }).environmentRace.calls);
  // On the old implementation both calls are in flight: finish the newest
  // first to expose the rollback. Serialization prevents that ordering.
  if ((await calls()).includes("a3")) {
    await finish(page, "a3"); await finish(page, "a2");
  } else {
    await finish(page, "a2");
    await expect.poll(calls).toEqual(["a2", "a3"]);
    await finish(page, "a3");
  }
  await page.getByRole("textbox", { name: "Request URL", exact: true }).fill("{{base_url}}/health");
  await expect(page.getByRole("button", { name: "Active variable environment" })).not.toHaveAttribute("aria-busy", "true");
  await page.getByRole("button", { name: "Send", exact: true }).first().click();
  await expect.poll(() => requested).toEqual(["https://race.example.test/a3/health"]);
  await expect(page.getByRole("button", { name: "Active variable environment" })).toHaveText("a3");
});
