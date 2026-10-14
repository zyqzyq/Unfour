import { expect, test } from "@playwright/test";

// Disposable browser fixtures verify the desktop UI contract, not Rust effects.
for (const locale of ["en", "zh-CN"] as const) {
  test(`Flow snapshots the active variable environment and records the explicit selection (${locale})`, async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript((value) => localStorage.setItem("unfour.locale", value), locale);
    await page.route("**/browser-mocks/workspace-environments.ts*", (route) => route.fulfill({
      contentType: "application/javascript", body: `
        let selected = "dev";
        export function listMockWorkspaceEnvironments(args) {
          return ["dev", "prod"].map((id) => ({ id, workspaceId: args.workspaceId,
            name: id === "dev" ? "Development variables" : "Production variables",
            isActive: selected === id, variables: [], sortOrder: 0, revision: 1,
            createdAt: "", updatedAt: "", deletedAt: null }));
        }
        export function setMockActiveEnvironment(args) {
          selected = args.environmentId;
          return listMockWorkspaceEnvironments(args);
        }
      `,
    }));
    await page.route("**/browser-mocks/flow.ts*", (route) => route.fulfill({
      contentType: "application/javascript", body: `
        import { UNHANDLED } from "./types.ts";
        const definition = { id: "flow-env", workspaceId: "mock-workspace", name: "Environment check",
          revision: 3, inputs: [], steps: [{ id: "wait", name: "Wait", kind: "wait", durationMs: 1, timeoutMs: 1000, next: null }] };
        let run = { id: "historical", flowId: definition.id, workspaceId: definition.workspaceId, definition,
          status: "succeeded", startedAt: "2026-10-14T00:00:00Z", finishedAt: "2026-10-14T00:00:01Z",
          context: { workspaceId: definition.workspaceId, flowId: definition.id, environmentId: "deleted-env",
            inputs: {}, initiator: "human", confirmEffects: true }, resources: {}, steps: [], error: null };
        export function handleFlowMock(command, args) {
          if (command === "flow_list") return [definition];
          if (command === "flow_runs_list") return [run];
          if (command === "flow_run_get") return run;
          if (command === "flow_run") {
            document.documentElement.dataset.flowRunInput = JSON.stringify(args);
            run = { ...run, id: "new-run", context: args.input };
            return run;
          }
          return UNHANDLED;
        }
      `,
    }));
    const text = locale === "en"
      ? { modules: "Modules", run: "Run", environment: "Variable environment", active: "Active variable environment", cancel: "Cancel", history: "Run history", detail: "Run detail", recorded: "Variable environment used (ID)", back: "Back to editor", only: "Workspace variables only" }
      : { modules: "模块", run: "运行", environment: "变量环境", active: "当前变量环境", cancel: "取消", history: "运行历史", detail: "运行详情", recorded: "使用的变量环境（ID）", back: "返回编辑态", only: "仅 Workspace 变量" };
    await page.goto("/");
    const active = page.getByRole("button", { name: text.active, exact: true });
    await expect(active).toHaveText("Development variables");
    await expect(active).toHaveAttribute("title", /API.*SSH Task.*Flow/);
    await expect(page.getByLabel(locale === "en" ? "Workspace type: DEV" : "工作区类型：DEV")).toHaveAttribute("title", /MCP/);
    await page.getByRole("navigation", { name: text.modules }).getByRole("button", { name: "Flow", exact: true }).click();
    await page.getByRole("button", { name: "Environment check", exact: true }).click();
    await page.getByRole("button", { name: text.history, exact: true }).click();
    await page.getByRole("button", { name: /2026-10-14T00:00:00Z/ }).click();
    await expect(page.getByRole("complementary", { name: text.detail })).toContainText(`${text.recorded}: deleted-env`);
    await page.screenshot({ animations: "disabled", path: `test-results/flow-environment-history-${locale}.png` });
    await page.getByRole("button", { name: text.back, exact: true }).click();

    await page.getByRole("button", { name: text.run, exact: true }).click();
    const dialog = page.getByRole("dialog", { name: text.run, exact: true });
    await expect(dialog.getByRole("combobox", { name: text.environment, exact: true })).toHaveValue("dev");
    await expect(dialog.getByText(`${text.environment}: Development variables (dev)`, { exact: true })).toBeVisible();
    await page.screenshot({ animations: "disabled", path: `test-results/flow-environment-default-${locale}.png` });
    await expect(page.locator("html")).not.toHaveAttribute("data-flow-run-input");
    await dialog.getByRole("combobox", { name: text.environment, exact: true }).selectOption("");
    await expect(dialog.getByText(`${text.environment}: ${text.only}`, { exact: true })).toBeVisible();
    await page.screenshot({ animations: "disabled", path: `test-results/flow-environment-${locale}.png` });
    await dialog.getByRole("button", { name: text.run, exact: true }).click();
    const input = await page.locator("html").getAttribute("data-flow-run-input");
    expect(JSON.parse(input ?? "null")).toMatchObject({ input: { environmentId: null, initiator: "human", confirmEffects: true }, expectedRevision: 3 });
    await expect(page.getByRole("complementary", { name: text.detail })).toContainText(`${text.recorded}: ${text.only}`);
    await page.getByRole("button", { name: text.back, exact: true }).click();

    await active.click();
    await page.getByRole("menuitem", { name: "Production variables", exact: true }).click();
    await expect(active).toHaveText("Production variables");
    await page.getByRole("button", { name: text.run, exact: true }).click();
    await expect(dialog.getByRole("combobox", { name: text.environment, exact: true })).toHaveValue("prod");
    await dialog.getByRole("button", { name: text.cancel, exact: true }).click();
    expect(errors).toEqual([]);
  });
}
