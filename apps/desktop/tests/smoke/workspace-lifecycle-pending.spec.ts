import { expect, test, type Page } from "@playwright/test";

type LifecycleRace = {
  calls: { command: string; args: Record<string, unknown> }[];
  pending: { resolve: () => void; reject: (error: Error) => void }[];
};

async function fixture(page: Page) {
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/state.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      mockWorkspace.name = "Original fixture";
      mockWorkspace.isDefault = false;
      mockState.workspaces.push({ ...mockWorkspace, id: "fallback", name: "Fallback fixture", isDefault: true });
    ` });
  });
  await page.route("**/browser-mocks/workspace.ts*", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: await response.text() + `
      window.lifecycleRace = { calls: [], pending: [] };
      for (const command of ["workspace_create", "workspace_delete"]) {
        const original = handlers.get(command);
        handlers.set(command, async (args) => {
          window.lifecycleRace.calls.push({ command, args });
          await new Promise((resolve, reject) => window.lifecycleRace.pending.push({ resolve, reject }));
          return original(args);
        });
      }
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow", exact: true }).click();
  await page.getByRole("button", { name: "New Flow", exact: true }).click();
  await page.getByRole("textbox", { name: "Flow name", exact: true }).fill("Preserved Flow draft");
}

async function calls(page: Page) {
  return page.evaluate(() => (window as unknown as { lifecycleRace: LifecycleRace }).lifecycleRace.calls);
}

async function finish(page: Page, fail = false) {
  await page.evaluate((fail) => {
    const pending = (window as unknown as { lifecycleRace: LifecycleRace }).lifecycleRace.pending.shift();
    if (!pending) throw new Error("Missing isolated pending lifecycle command");
    if (fail) pending.reject(new Error("Isolated lifecycle failure")); else pending.resolve();
  }, fail);
}

for (const action of ["create", "delete"] as const) {
  for (const outcome of ["success", "retry", "cancel"] as const) {
    test(`${action} locks the pending modal and handles ${outcome}`, async ({ page }, testInfo) => {
      await fixture(page);
      const flow = page.getByLabel("Flow name", { exact: true });
      await page.getByRole("button", { name: /Original fixture/ }).click();
      await page.getByRole("menuitem", { name: action === "create" ? "New workspace" : "Delete current", exact: true }).click();
      const dialog = page.getByRole("dialog", { name: action === "create" ? "New workspace" : "Delete workspace", exact: true });
      const submit = dialog.getByRole("button", { name: action === "create" ? "Create" : "Delete", exact: true });
      if (action === "create") {
        await dialog.getByRole("textbox").fill("Created fixture");
        await dialog.getByRole("combobox").selectOption("test");
        await dialog.getByRole("textbox").press("Enter");
      } else await submit.click();
      const leave = page.getByRole("dialog", { name: "Leave workspace?", exact: true });
      await expect(leave.getByRole("button", { name: "Cancel", exact: true })).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(leave).toHaveCount(0);
      expect(await calls(page)).toEqual([]);
      await expect(flow).toHaveValue("Preserved Flow draft");
      await submit.click();
      await leave.getByRole("button", { name: "Discard and leave", exact: true }).click();
      await expect.poll(async () => (await calls(page)).length).toBe(1);
      await expect(submit).toBeDisabled();

      // Physical pointer events exercise the overlay and disabled control paths.
      const cancel = dialog.getByRole("button", { name: "Cancel", exact: true });
      const cancelBox = await cancel.boundingBox();
      if (!cancelBox) throw new Error("Missing cancel button bounds");
      await page.mouse.click(cancelBox.x + cancelBox.width / 2, cancelBox.y + cancelBox.height / 2);
      await expect(dialog).toBeVisible();
      await expect(cancel).toBeDisabled();
      await page.keyboard.press("Escape");
      await expect(dialog).toBeVisible();
      await page.mouse.click(5, 5);
      await expect(dialog).toBeVisible();
      const flowBox = await flow.boundingBox();
      if (!flowBox) throw new Error("Missing Flow editor bounds");
      await page.mouse.click(flowBox.x + flowBox.width / 2, flowBox.y + flowBox.height / 2);
      await page.keyboard.type("must not reach Flow");
      await page.keyboard.press("Tab");
      await page.keyboard.press("Enter");
      await expect(flow).toHaveValue("Preserved Flow draft");
      await expect(flow).not.toBeFocused();
      await expect(dialog).toBeVisible();
      await expect(dialog).toBeFocused();
      if (action === "create") {
        await expect(dialog.getByRole("textbox")).toBeDisabled();
        await expect(dialog.getByRole("textbox")).toHaveValue("Created fixture");
        await expect(dialog.getByRole("combobox")).toBeDisabled();
        await expect(dialog.getByRole("combobox")).toHaveValue("test");
        await dialog.locator("form").evaluate((form: HTMLFormElement) => { form.requestSubmit(); form.requestSubmit(); });
      }
      await submit.dispatchEvent("click");
      expect(await calls(page)).toHaveLength(1);
      await page.screenshot({ path: testInfo.outputPath(`${action}-pending.png`), animations: "disabled" });

      if (outcome !== "success") {
        await finish(page, true);
        await expect(page.getByText(/Isolated lifecycle failure/).first()).toBeVisible();
        await expect(submit).toBeEnabled();
        await expect(cancel).toBeEnabled();
        await expect(page.getByRole("button", { name: /Original fixture/, includeHidden: true })).toBeVisible();
        await expect(flow).toHaveValue("Preserved Flow draft");
        if (action === "create") {
          await expect(dialog.getByRole("textbox")).toBeEnabled();
          await expect(dialog.getByRole("textbox")).toHaveValue("Created fixture");
          await expect(dialog.getByRole("combobox")).toBeEnabled();
          await expect(dialog.getByRole("combobox")).toHaveValue("test");
        }
        if (outcome === "cancel") {
          await cancel.click();
          await expect(dialog).toHaveCount(0);
          await flow.fill("Continued original Flow");
          await expect(flow).toHaveValue("Continued original Flow");
          expect(await calls(page)).toHaveLength(1);
          return;
        }
        await submit.click();
        await leave.getByRole("button", { name: "Discard and leave", exact: true }).click();
        await expect.poll(async () => (await calls(page)).length).toBe(2);
      }
      await finish(page);
      await expect(dialog).toHaveCount(0);
      await expect(page.getByRole("button", { name: action === "create" ? /Created fixture/ : /Fallback fixture/ })).toBeVisible();
      await expect(leave).toHaveCount(0);
      await expect(flow).toHaveCount(0);
      const recorded = await calls(page);
      expect(recorded).toHaveLength(outcome === "retry" ? 2 : 1);
      expect(recorded.every((call) => call.command === `workspace_${action}`)).toBe(true);
      expect(recorded[0].args).toMatchObject(action === "create" ? { name: "Created fixture", environmentType: "test" } : { workspaceId: "mock-workspace" });
    });
  }
}
