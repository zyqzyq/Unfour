// UI-only acceptance checks. Runtime targets and inputs must remain in memory.
import assert from "node:assert/strict";
import { expect } from "@playwright/test";

export async function verifyTemporaryTasks(page, received, terminalLabel, screenshotPath) {
  const authCount = received.filter((line) => line === "FIXTURE_AUTH_OK").length;
  const cases = [
    { command: "fixture-task-ok", mode: "exec", result: "success", marker: "FIXTURE_TASK_OK", output: "UNFOUR_TASK_STDOUT" },
    { command: "fixture-task-wait", mode: "exec", result: "cancel", marker: "FIXTURE_TASK_WAITING", output: "UNFOUR_TASK_WAITING" },
    { command: "fixture-task-login", mode: "exec", result: "login", marker: "FIXTURE_TASK_LOGIN" },
    { command: "fixture-terminal-ok", mode: "terminal", result: "success", marker: "FIXTURE_TERMINAL_TASK_OK", output: "UNFOUR_TERMINAL_STDOUT" },
    { command: "fixture-terminal-fail", mode: "terminal", result: "fail", marker: "FIXTURE_TERMINAL_TASK_FAIL" },
    { command: "fixture-terminal-wait", mode: "terminal", result: "cancel", marker: "FIXTURE_TERMINAL_TASK_WAIT", output: "UNFOUR_TERMINAL_WAITING" },
  ];
  const sentinels = [...cases.map((entry) => entry.command), "UNFOUR_TASK_STDOUT", "UNFOUR_TASK_STDERR", "UNFOUR_TASK_WAITING", "UNFOUR_TERMINAL_STDOUT", "UNFOUR_TERMINAL_STDERR", "UNFOUR_TERMINAL_WAITING"];
  await page.getByRole("tab", { name: "Tasks", exact: true }).click();
  for (const [index, test] of cases.entries()) {
    const { command } = test;
    const name = `Bastion temporary task QA ${index + 1}`;
    await page.getByRole("button", { name: /^(New Task|新建 Task)$/ }).first().click();
    const editor = page.locator("fieldset:visible").filter({ has: page.locator("#ssh-task-name") });
    await expect(editor).toBeVisible();
    const save = editor.getByRole("button", { name: /^(Save|保存)$/ });
    await expect(save).toBeDisabled();
    await expect(editor.getByRole("status")).toHaveText(/Enter a task name before saving or running\.|请先填写 Task 名称再保存或运行。/);
    await editor.locator("#ssh-task-name").fill(name);
    await expect(editor.locator("#ssh-task-default-connection")).toHaveValue("");
    await expect(editor.locator("#ssh-task-default-connection option")).toHaveCount(1);
    await editor.getByRole("button", { name: /^(Add step|添加步骤)$/ }).first().click();
    await page.getByRole("menuitem", { name: /^(Add Command|添加 Command)$/ }).click();
    // Store only a placeholder. Its rendered command is also a persistence
    // sentinel, so the main check can detect accidental runtime context writes.
    await editor.locator("textarea").fill("{{COMMAND}}");
    await expect(save).toBeEnabled();
    await save.click();
    await expect(save).toBeEnabled();
    await editor.getByRole("button", { name: /^(Run|运行)$/ }).click();
    const dialog = page.getByRole("dialog", { name: /^(Run SSH Task|运行 SSH Task)$/ });
    await expect(dialog).toBeVisible();
    const targets = dialog.getByRole("combobox", { name: /^(SSH connection|SSH 连接)$/ });
    await expect.poll(() => targets.locator('option[value^="temporary:"]').count()).toBe(1);
    const target = targets.locator('option[value^="temporary:"]').first();
    assert.ok((await target.textContent()).includes(terminalLabel), "target must identify the live node and endpoint");
    await targets.selectOption(await target.getAttribute("value"));
    await dialog.getByRole("combobox", { name: /^(Execution mode|执行方式)$/ }).selectOption(test.mode);
    await dialog.getByLabel("COMMAND", { exact: true }).fill(command);
    await expect(dialog.getByRole("note").filter({ hasText: /Closing its parent session|关闭父会话/ })).toBeVisible();
    if (test.mode === "terminal") {
      await expect(dialog.getByRole("button", { name: /^(Run|运行)$/ })).toBeDisabled();
      await dialog.getByRole("checkbox", { name: /idle POSIX shell|空闲.*POSIX|POSIX.*空闲/ }).check();
      if (test.result === "success" && screenshotPath) await page.screenshot({ path: screenshotPath.replace(/\.png$/, "-terminal-target.png"), animations: "disabled" });
    }
    if (index === 0 && screenshotPath) await page.screenshot({ path: screenshotPath.replace(/\.png$/, "-target.png"), animations: "disabled" });
    await dialog.getByRole("button", { name: /^(Run|运行)$/ }).click();
    await expect(dialog).toHaveCount(0);
    const output = page.getByLabel(/^(Task output|任务输出)$/);
    const panel = page.locator("section").filter({ has: output }).last();
    await expect(panel.getByText(/^(Temporary run · memory only|临时运行 · 仅内存)$/)).toBeVisible();
    if (test.output) await expect(output).toContainText(test.output);
    if (test.result === "success") {
      await expect(output).toContainText(test.mode === "terminal" ? "UNFOUR_TERMINAL_STDERR" : "UNFOUR_TASK_STDERR");
      await expect(panel.getByText(/^(Success|成功)$/).first()).toBeVisible();
    } else if (test.result === "fail") {
      await expect(panel.getByText(/^(Failed|失败)$/).first()).toBeVisible();
      await expect(output).toContainText("23");
    } else {
      if (test.result === "login") await expect(output).toContainText(/login prompt|登录提示/);
      if (test.mode === "terminal") {
        await page.getByRole("tab", { name: "Connections", exact: true }).click();
        await page.getByRole("tab", { name: terminalLabel, exact: true }).click();
        await expect(page.getByText(/Terminal in use|终端.*占用/)).toBeVisible();
        const before = received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length;
        await page.locator(".xterm-helper-textarea:visible").last().focus();
        await page.keyboard.type("check");
        await page.keyboard.press("Enter");
        await page.waitForTimeout(250);
        assert.equal(received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length, before,
          "manual input must not reach a task-owned terminal");
        await page.getByRole("tab", { name: "Tasks", exact: true }).click();
      }
      await panel.getByRole("button", { name: /^(Cancel Run|取消运行)$/ }).click();
      // Closing an exec channel cannot confirm that the remote process stopped.
      await expect(panel.getByText(/^(Outcome unknown|结果未知)$/).first()).toBeVisible();
      await expect(panel.getByRole("button", { name: /^(Cancel Run|取消运行)$/ })).toHaveCount(0);
    }
    await expect.poll(() => received.filter((line) => line === test.marker).length).toBe(1);
    await expect(output).not.toContainText("UNFOUR_TASK_V2");
    await expect(output).not.toContainText("stty");
    if (test.mode === "terminal" && test.result === "success" && screenshotPath) await page.screenshot({ path: screenshotPath.replace(/\.png$/, "-terminal.png"), animations: "disabled" });
    assert.equal(received.filter((line) => line === "FIXTURE_AUTH_OK").length, authCount,
      "temporary Task must reuse the authenticated transport");
    await expect(panel.getByRole("button", { name: /^(Open log|打开日志)$/ })).toHaveCount(0);
    await panel.getByRole("button", { name: /^(Run again|重新运行)$/ }).click();
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("status")).toContainText(/Temporary runs do not save their target or inputs|临时运行不保存目标和参数/);
    await expect(targets).toHaveValue("");
    await expect(dialog.getByLabel("COMMAND", { exact: true })).toHaveValue("");
    await dialog.getByRole("button", { name: /^(Use previous parameters|使用上次参数)$/ }).click();
    await expect(dialog.getByRole("status")).toContainText(/Temporary runs do not save their target or inputs|临时运行不保存目标和参数/);
    await expect(targets).toHaveValue("");
    await expect(dialog.getByLabel("COMMAND", { exact: true })).toHaveValue("");
    await dialog.getByRole("button", { name: /^(Cancel|取消)$/ }).click();
    await expect(page.locator('[role="dialog"]')).toHaveCount(0);
    if (index === 0 && screenshotPath) await page.screenshot({ path: screenshotPath, animations: "disabled" });
    await panel.getByRole("button", { name: /^(Close|关闭)$/ }).click();
  }
  await page.getByRole("tab", { name: "Connections", exact: true }).click();
  await page.getByRole("tab", { name: terminalLabel, exact: true }).click();
  const before = received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length;
  await page.locator(".xterm-helper-textarea:visible").last().focus();
  await page.keyboard.type("check");
  await page.keyboard.press("Enter");
  await expect.poll(() => received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length).toBe(before + 1);
  console.log("Temporary Task success/stdout/stderr/cancel passed with no saved connection or reauthentication; recall is empty and parent PTY still replies");
  // Run IDs are deliberately not exposed in the product UI. The main verifier
  // also checks that the run table and log directory are empty after app exit.
  return { createdTasks: cases.length, runIds: [], sentinels };
}
