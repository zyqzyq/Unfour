import { expect, test, type Locator } from "@playwright/test";

for (const locale of ["en", "zh-CN"] as const) {
  test(`${locale}: SSH load errors preserve cached data in small panels`, async ({ page }, testInfo) => {
    const zh = locale === "zh-CN";
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.addInitScript(({ locale, theme }) => {
      localStorage.setItem("unfour.locale", locale);
      localStorage.setItem("unfour.theme", theme);
    }, { locale, theme: zh ? "dark" : "light" });
    // Expose refetch only in this browser response; exercise the real query
    // cache and retry wiring without changing production code or running SSH.
    await page.route("**/src/main.tsx*", async (route) => {
      const response = await route.fetch();
      await route.fulfill({ response, body: await response.text() + `
        window.__sshRefresh = () => Promise.all([
          queryClient.refetchQueries({ queryKey: ["ssh-tasks"] }),
          queryClient.refetchQueries({ queryKey: ["ssh-task-runs"] }),
        ]);
      ` });
    });
    await page.route("**/browser-mocks/ssh-tasks.ts*", async (route) => {
      await route.fulfill({ contentType: "application/javascript", body: `
        import { UNHANDLED } from "./types.ts";
        const workspaceId = "mock-workspace";
        const task = { id: "task", workspaceId, name: "Layout fixture", description: "", sortOrder: 0, createdAt: "now", updatedAt: "now", deletedAt: null };
        const detail = { task, steps: [], localBinding: { taskId: "task", workspaceId, defaultConnectionId: null, lastUsedConnectionId: null } };
        const run = { id: "run", workspaceId, taskId: "task", connectionId: null, status: "failed", startedAt: "2026-10-21T00:00:00Z", finishedAt: "2026-10-21T00:00:01Z", logPath: "fixture.log", errorMessage: "Cached failure" };
        window.__sshLoadMode = "success";
        window.__sshEmpty = false;
        const pending = [];
        const result = (command) => window.__sshEmpty ? [] : command === "ssh_tasks_list" ? [task] : [run];
        window.__sshFinishRetry = () => {
          window.__sshLoadMode = "success";
          pending.splice(0).forEach(({ command, resolve }) => resolve(result(command)));
        };
        export function handleSshTaskMock(command) {
          if (command === "ssh_task_get") return detail;
          if (command !== "ssh_tasks_list" && command !== "ssh_task_runs_list") return UNHANDLED;
          if (window.__sshLoadMode === "fail") throw new Error("Load failure fixture");
          if (window.__sshLoadMode === "pending") return new Promise((resolve) => pending.push({ command, resolve }));
          return result(command);
        }
      ` });
    });
    await page.setViewportSize({ width: 960, height: 600 });
    await page.goto("/");
    await page.getByRole("navigation", { name: zh ? "模块" : "Modules" })
      .getByRole("button", { name: zh ? "SSH 终端" : "SSH Terminal", exact: true }).click();
    await page.getByRole("tab", { name: "Tasks", exact: true }).click();
    const task = page.getByRole("treeitem").filter({ hasText: "Layout fixture" });
    await task.getByRole("button", { name: "Layout fixture", exact: true }).click();
    await page.getByRole("radio", { name: zh ? "运行历史" : "Run History", exact: true }).click();
    const table = page.getByRole("table");
    await expect(table).toHaveText(/Cached failure/);
    const history = table.locator("../..");
    const separator = page.getByRole("separator", { name: "Resize vertical split (double-click to reset)" });
    const historyBox = (await history.boundingBox())!;
    const separatorBox = (await separator.boundingBox())!;
    const dragX = separatorBox.x + separatorBox.width / 2;
    await page.mouse.move(dragX, separatorBox.y + separatorBox.height / 2);
    await page.mouse.down();
    await page.mouse.move(dragX, historyBox.y + historyBox.height - 140, { steps: 10 });
    await page.mouse.up();
    await expect.poll(async () => (await history.boundingBox())?.height).toBeCloseTo(140, 0);

    await page.evaluate('window.__sshLoadMode = "fail"; window.__sshRefresh()');
    const historyAlert = history.getByRole("alert");
    const sidebar = page.getByRole("complementary");
    const listAlert = sidebar.getByRole("alert");
    await expect(historyAlert).toHaveText(zh ? "无法加载运行历史。" : "Unable to load task history.");
    await expect(listAlert).toHaveText(zh ? "无法加载 SSH Tasks。" : "Unable to load SSH tasks.");
    for (const alert of [historyAlert, listAlert]) {
      const bar = alert.locator("../..");
      expect((await bar.boundingBox())!.height).toBeLessThanOrEqual(34);
      expect(await bar.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
      await expect(bar.getByRole("button", { name: zh ? "重试" : "Retry", exact: true })).toBeInViewport({ ratio: 1 });
    }
    await expect(task).toBeVisible();
    const header = table.locator("thead");
    const firstRow = table.locator("tbody tr").first();
    await expectInside(header, history);
    await expectInside(firstRow, history);
    await page.screenshot({ path: testInfo.outputPath("cached-error-140px.png"), animations: "disabled" });

    await page.evaluate('window.__sshLoadMode = "pending"');
    const historyRetry = history.getByRole("button", { name: zh ? "重试" : "Retry", exact: true });
    const listRetry = sidebar.getByRole("button", { name: zh ? "重试" : "Retry", exact: true });
    await historyRetry.click();
    await listRetry.click();
    await expect(historyRetry).toBeDisabled();
    await expect(listRetry).toBeDisabled();
    await expect(task).toBeVisible();
    await expectInside(firstRow, history);
    await page.evaluate("window.__sshFinishRetry()");
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(firstRow).toContainText("Cached failure");

    // Successful empty responses remove cached records; a later failure must
    // retain the full error state instead of showing an empty-data message.
    await page.evaluate("window.__sshEmpty = true; window.__sshRefresh()");
    await expect(table).toHaveCount(0);
    await page.evaluate('window.__sshLoadMode = "fail"; window.__sshRefresh()');
    for (const message of [
      zh ? "无法加载运行历史。" : "Unable to load task history.",
      zh ? "无法加载 SSH Tasks。" : "Unable to load SSH tasks.",
    ]) {
      const fullError = page.getByRole("alert").filter({ hasText: message }).locator("../..");
      await expect(fullError).toBeVisible();
      expect((await fullError.boundingBox())!.height).toBeGreaterThanOrEqual(96);
      await expect(fullError.getByRole("button", { name: zh ? "重试" : "Retry", exact: true })).toBeEnabled();
    }
    expect(errors).toEqual([]);
  });
}

async function expectInside(child: Locator, container: Locator) {
  const childBox = (await child.boundingBox())!;
  const containerBox = (await container.boundingBox())!;
  expect(childBox.y).toBeGreaterThanOrEqual(containerBox.y);
  expect(childBox.y + childBox.height).toBeLessThanOrEqual(containerBox.y + containerBox.height);
}
