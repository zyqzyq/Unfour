// Two synthetic failures on one logged-in terminal, using the actual EXE UI.
import assert from "node:assert/strict";
import { expect } from "@playwright/test";

const count = (lines, marker) => lines.filter((line) => line === marker).length;

export async function verifyCurlRecovery(page, received, screenshotPath) {
  const auths = count(received, "FIXTURE_AUTH_OK");
  const sentinels = [];
  for (const failure of [
    { code: "TERMINAL_TOOLS_MISSING", marker: "FIXTURE_RAW_TOOL_FAILURE", interrupts: 0 },
    { code: "SSH_TERMINAL_END_TIMEOUT", marker: "FIXTURE_RAW_END_WITHHELD", interrupts: 1 },
  ]) {
    const interrupted = count(received, "FIXTURE_PTY_INTERRUPT");
    const restored = count(received, "FIXTURE_RAW_RESTORED");
    const curlChecks = count(received, "FIXTURE_CURL_PROBE");
    await page.getByRole("button", { name: /^(Capability checks|能力检测)$/ }).click();
    const dialog = page.getByRole("dialog", { name: /^(Capability checks|能力检测)$/ });
    await dialog.getByRole("combobox", { name: /^(Check to run|检测项目)$/ }).selectOption("curl");
    await dialog.getByRole("checkbox", { name: /authorized test asset|已获授权的测试资产/ }).check();
    await dialog.getByRole("checkbox", { name: /idle POSIX shell|空闲.*POSIX|POSIX.*空闲/ }).check();
    const reportView = dialog.getByTestId("temporary-capability-report");
    const previousRun = await reportView.count()
      ? JSON.parse(await reportView.textContent()).reports.find((item) => item.kind === "curl")?.runId : null;
    await dialog.getByRole("button", { name: /^(Run check|开始检测)$/ }).click();
    let report;
    await expect.poll(async () => {
      const text = await dialog.getByTestId("temporary-capability-report").textContent().catch(() => null);
      report = text ? JSON.parse(text).reports.find((item) => item.kind === "curl") : null;
      return report?.runId !== previousRun ? report?.status : null;
    }, { timeout: 45000 }).toBe("completed");
    assert.equal(report.checks[0].code, failure.code);
    assert.deepEqual(report.checks.map(({ id }) => id), ["transport-roundtrip", "transport-recovery"]);
    assert.deepEqual(report.checks[1], { id: "transport-recovery", status: "pass", code: "OK", expectedExitCode: null, actualExitCode: null });
    assert.equal(count(received, "FIXTURE_CURL_PROBE"), curlChecks, "transport failure must prevent the curl tools stage");
    await expect(dialog.getByText(/curl tools check was not run|未执行 curl 工具检查/)).toBeVisible();
    sentinels.push(report.runId);
    assert.equal(count(received, failure.marker), 1);
    assert.equal(count(received, "FIXTURE_RAW_RESTORED"), restored + 1);
    assert.equal(count(received, "FIXTURE_PTY_INTERRUPT"), interrupted + failure.interrupts);
    assert.equal(count(received, "FIXTURE_AUTH_OK"), auths);
    if (screenshotPath) await page.screenshot({ path: screenshotPath.replace(/\.png$/, `-${failure.code}.png`), animations: "disabled" });
    await dialog.getByRole("button", { name: /^(Close|关闭)$/ }).last().click();
    await expect(page.getByRole("status").filter({ hasText: /^(Terminal in use|终端正被)/ })).toHaveCount(0);
    const checks = count(received, "FIXTURE_ROUNDTRIP_OK");
    await page.locator(".xterm-helper-textarea:visible").last().focus();
    await page.keyboard.type("check");
    await page.keyboard.press("Enter");
    await expect.poll(() => count(received, "FIXTURE_ROUNDTRIP_OK")).toBe(checks + 1);
    console.log(`PASS: ${failure.code} kept its diagnosis, restored original terminal, and used ${failure.interrupts} interrupts`);
  }
  return { sentinels };
}
