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
  await page.getByRole("button", { name: "Run again", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Run", exact: true })).toBeVisible();
  await page.getByRole("dialog").getByRole("button", { name: "Cancel", exact: true }).click();
  for (const locale of ["en", "zh-CN"]) {
    await page.evaluate((value) => localStorage.setItem("unfour.locale", value), locale);
    await page.setViewportSize({ width: 900, height: 760 });
    await page.reload();
    await page.getByRole("navigation", { name: locale === "en" ? "Modules" : "模块" }).getByRole("button", { name: "Flow", exact: true }).click();
    await page.getByRole("button", { name: "Release check", exact: true }).click();
    await page.getByRole("button", { name: locale === "en" ? "Run history" : "运行历史" }).click();
    await page.getByRole("button", { name: /2026-09-21T10:00:00Z/ }).click();
    await page.getByLabel(locale === "en" ? "Recorded node" : "历史运行节点").selectOption("wait");
    const narrowDetail = page.getByRole("complementary", { name: locale === "en" ? "Run detail" : "运行详情" });
    await expect(narrowDetail.getByRole("button", { name: locale === "en" ? "Edit this step" : "修改此步骤" })).toBeVisible();
    const canvasBox = await page.getByLabel(locale === "en" ? "Flow Canvas" : "Flow 画布", { exact: true }).boundingBox();
    const detailBox = await narrowDetail.boundingBox();
    expect(canvasBox && detailBox && detailBox.y >= canvasBox.y + canvasBox.height - 1).toBeTruthy();
    expect(await page.locator(".flow-page").evaluate((element) => element.scrollWidth <= element.clientWidth)).toBeTruthy();
    await page.screenshot({ animations: "disabled", path: `test-results/flow-run-narrow-${locale}.png` });
  }
  expect(errors).toEqual([]);
});

test("failed run navigation and long diagnostics remain usable in a narrow window", async ({ page }) => {
  const stepName = "Deploy_" + "long_name_".repeat(24);
  const error = "SSH_CONNECTION_FAILED: " + "diagnostic_".repeat(80);
  await page.route("**/browser-mocks/flow.ts*", async (route) => {
    await route.fulfill({ contentType: "application/javascript", body: `
      import { UNHANDLED } from "./types.ts";
      const definition = { id: "long", workspaceId: "mock-workspace", name: "Long diagnostics", revision: 1, inputs: [], steps: [
        { id: "step", name: ${JSON.stringify(stepName)}, kind: "wait", durationMs: 1, timeoutMs: 1000, next: null }
      ] };
      const run = { id: "failed-run", flowId: definition.id, workspaceId: definition.workspaceId, definition,
        status: "failed", startedAt: "2026-09-27T00:00:00Z", finishedAt: "2026-09-27T00:00:01Z", error: null,
        context: { workspaceId: definition.workspaceId, flowId: definition.id, inputs: {}, initiator: "human", confirmEffects: true }, resources: {},
        steps: [{ stepId: "step", status: "failed", durationMs: 1000, error: ${JSON.stringify(error)}, attempts: [] }] };
      export function handleFlowMock(command) {
        if (command === "flow_list") return [definition];
        if (command === "flow_runs_list") return [run];
        if (command === "flow_run_get") return run;
        return UNHANDLED;
      }
    ` });
  });
  await page.setViewportSize({ width: 900, height: 760 });
  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "Flow" }).click();
  await page.getByRole("button", { name: "Long diagnostics", exact: true }).click();
  await page.getByRole("button", { name: "Run history" }).click();
  await page.getByRole("button", { name: /2026-09-27T00:00:00Z/ }).click();
  await page.getByRole("button", { name: `Locate: ${stepName} · Failed`, exact: true }).click();
  const detail = page.getByRole("complementary", { name: "Run detail" });
  await expect(detail.getByRole("alert")).toHaveText(error);
  expect(await page.locator(".flow-page").evaluate((element) => element.scrollWidth <= element.clientWidth)).toBeTruthy();
  expect(await detail.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBeTruthy();
  await detail.getByRole("button", { name: "Edit this step" }).click();
  await expect(page.getByLabel("Step name", { exact: true })).toHaveValue(stepName);
  await page.screenshot({ animations: "disabled", path: "test-results/flow-long-name-editor.png" });
});
