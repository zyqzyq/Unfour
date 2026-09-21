import { expect, test } from "@playwright/test";

// UI-only fixture: no Rust execution or persistence is exercised by this test.
test("Run dialog and History keep execution details beside the Canvas", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.route("**/browser-mocks/flow.ts*", async (route) => {
    await route.fulfill({ contentType: "application/javascript", body: `
      import { UNHANDLED } from "./types.ts";
      let definition;
      let run;
      export function handleFlowMock(command, args) {
        if (command === "flow_list") {
          definition = { id: "fixture", workspaceId: args.workspaceId, name: "Release check", revision: 3,
            inputs: [{ name: "version", type: "number", required: true, secret: false }, { name: "credential", type: "string", required: false, secret: true }],
            steps: [{ id: "wait", name: "Wait for release", kind: "wait", durationMs: 1000, timeoutMs: 2000, next: null }] };
          run = { id: "run-fixture", flowId: definition.id, workspaceId: args.workspaceId, definition,
            status: "succeeded", startedAt: "2026-09-21T10:00:00Z", finishedAt: "2026-09-21T10:00:01Z", error: null,
            context: { workspaceId: args.workspaceId, flowId: definition.id, environmentId: null, inputs: { version: 42 }, initiator: "human", confirmEffects: true }, resources: {},
            steps: [{ stepId: "wait", status: "succeeded", durationMs: 1000, startedAt: "2026-09-21T10:00:00Z", error: null, output: { ready: true }, attempts: [{ number: 1, input: { version: 42 }, output: { ready: true }, error: null, durationMs: 1000 }] }] };
          return [definition];
        }
        if (command === "flow_runs_list") return [run];
        if (command === "flow_run_get" || command === "flow_run") return run;
        return UNHANDLED;
      }
    ` });
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "Release check", exact: true }).click();
  await expect(page.getByText("Run inputs & history")).toHaveCount(0);
  await page.getByRole("button", { name: "Run", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Run", exact: true });
  await expect(dialog.getByRole("button", { name: "Run", exact: true })).toBeDisabled();
  await dialog.getByLabel("version", { exact: true }).fill("42");
  await dialog.getByLabel("credential", { exact: true }).fill("fixture-secret");
  await expect(dialog.getByLabel("credential", { exact: true })).toHaveAttribute("type", "password");
  await expect(dialog.getByLabel("Input preview (masked)")).not.toContainText("fixture-secret");
  await page.screenshot({ animations: "disabled", path: "test-results/flow-run-dialog.png" });
  await dialog.getByRole("button", { name: "Run", exact: true }).click();
  await expect(page.getByText("Viewing run", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Back to editor" })).toBeFocused();
  await page.locator(".react-flow__node").filter({ hasText: "Wait for release" }).click();
  const inspector = page.getByRole("complementary", { name: "Run detail" });
  await expect(inspector.getByText("Latest result", { exact: true })).toBeVisible();
  await expect(inspector.getByText("1 attempts", { exact: false })).toBeVisible();
  await expect(page.locator('.flow-canvas-node[data-status="succeeded"]')).toHaveCount(1);
  await expect(page.getByLabel("Flow name")).toBeDisabled();
  await expect(page.getByRole("button", { name: /Insert node/ })).toHaveCount(0);
  const canvas = await page.getByLabel("Flow Canvas", { exact: true }).boundingBox();
  const detail = await inspector.boundingBox();
  expect(canvas && detail && detail.x >= canvas.x + canvas.width - 1).toBeTruthy();
  await page.screenshot({ animations: "disabled", path: "test-results/flow-run-inspector.png" });
  await page.getByRole("button", { name: "Back to editor" }).click();
  await expect(page.getByLabel("Step name", { exact: true })).toBeVisible();
  await expect(page.locator(".flow-canvas-node[data-status]")).toHaveCount(0);
  await page.getByRole("button", { name: "Run history" }).click();
  const history = page.getByRole("dialog", { name: "Run history" });
  await expect(history).toContainText("1000 ms");
  await page.screenshot({ animations: "disabled", path: "test-results/flow-run-history.png" });
  await history.getByRole("button", { name: /2026-09-21T10:00:00Z/ }).click();
  await expect(page.getByText("Viewing run", { exact: true })).toBeVisible();
  await expect(history).not.toBeVisible();
  expect(errors).toEqual([]);
});
