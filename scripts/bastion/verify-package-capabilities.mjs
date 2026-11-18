// UI-only checks against the packaged app and a synthetic loopback SSH server.
import assert from "node:assert/strict";
import { expect } from "@playwright/test";

export async function verifyTerminalUx(page, received) {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  const input = page.locator(".xterm-helper-textarea:visible").last();
  await page.bringToFront();
  await input.focus();
  await expect.poll(() => page.locator(".xterm-cursor-blink:visible").last()
    .evaluate((element) => getComputedStyle(element).animationDuration)).toBe("1.4s");

  // Model WebView2's permission handoff: the first read rejects while the
  // permission query already reports granted. This never reads the OS clipboard.
  // Real Windows permission popup behavior remains part of the field checklist.
  await page.evaluate(() => {
    window.__a4Clipboard = {
      readText: navigator.clipboard.readText,
      query: navigator.permissions.query,
      reads: 0,
    };
    navigator.clipboard.readText = async () => {
      window.__a4Clipboard.reads += 1;
      if (window.__a4Clipboard.reads === 1) throw new DOMException("synthetic permission handoff", "NotAllowedError");
      return "check\r";
    };
    navigator.permissions.query = async (descriptor) => descriptor.name === "clipboard-read"
      ? { state: "granted" }
      : window.__a4Clipboard.query.call(navigator.permissions, descriptor);
  });
  const before = received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length;
  try {
    await page.locator(".xterm-screen:visible").last().click({ button: "right" });
    await page.getByRole("menuitem", { name: /^(Paste|粘贴)(?:\s|$)/ }).last().click();
    await expect.poll(() => received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length).toBe(before + 1);
    assert.equal(await page.evaluate(() => window.__a4Clipboard.reads), 2);
    await expect(page.getByText(/Failed to read from the clipboard|读取剪贴板失败/, { exact: true })).toHaveCount(0);
  } finally {
    await page.evaluate(() => {
      navigator.clipboard.readText = window.__a4Clipboard.readText;
      navigator.permissions.query = window.__a4Clipboard.query;
      delete window.__a4Clipboard;
    });
  }
  console.log("Packaged cursor cadence is 1.4s; a granted permission handoff retries once and pastes exactly once");
}

export async function verifyCapabilityChecks(page, received, metadata, sentinels, screenshotPath) {
  const authCount = received.filter((line) => line === "FIXTURE_AUTH_OK").length;
  const before = received.length;
  await page.getByRole("button", { name: /^(Capability checks|能力检测)$/ }).click();
  const dialog = page.getByRole("dialog", { name: /^(Capability checks|能力检测)$/ });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("combobox", { name: /^(Check to run|检测项目)$/ })).toHaveValue("transport");
  await expect(dialog.locator('option[value="terminal"]')).toHaveText(/SSH Task/);
  const run = dialog.getByRole("button", { name: /^(Run check|开始检测)$/ });
  await expect(run).toBeDisabled();
  assert.equal(received.slice(before).filter((line) => /^FIXTURE_(EXEC|TERMINAL|TRANSPORT|CURL|RAW|SFTP|THREE_CHANNELS)/.test(line)).length, 0,
    "opening the dialog must not run any probe");
  let bundle;
  const runSentinels = [];
  for (const variant of ["exec", "terminal", "channels", "sftp", "tcp", "tcp-denied"]) {
    const kind = variant === "tcp-denied" ? "tcp" : variant;
    await dialog.getByRole("combobox", { name: /^(Check to run|检测项目)$/ }).selectOption(kind);
    await expect(run).toBeDisabled();
    if (kind === "tcp") {
      await dialog.getByRole("textbox", { name: /^(Target host or IP|目标主机或 IP)$/ }).fill(variant === "tcp-denied" ? "denied.fixture.invalid" : "tcp.fixture.invalid");
      await dialog.getByRole("textbox", { name: /^(Port|端口)$/ }).fill("32123");
    }
    await dialog.getByRole("checkbox", { name: /^(I confirm this is an authorized test asset\.|我确认当前会话连接的是已获授权的测试资产。)$/ }).check();
    if (kind === "sftp") {
      await expect(run).toBeDisabled();
      await dialog.getByRole("checkbox", { name: /^(I authorize creating, reading and deleting the probe file in \/tmp\.|我授权在 \/tmp 创建、读取并删除检测文件。)$/ }).check();
    }
    if (kind === "terminal") {
      await expect(run).toBeDisabled();
      await dialog.getByRole("checkbox", { name: /idle POSIX shell|空闲.*POSIX|POSIX.*空闲/ }).check();
    }
    await expect(run).toBeEnabled();
    const previousId = bundle?.reports.find((report) => report.kind === kind)?.runId;
    await run.click();
    await expect.poll(async () => {
      const raw = await dialog.getByTestId("temporary-capability-report").textContent().catch(() => null);
      if (!raw) return null;
      bundle = JSON.parse(raw);
      const current = bundle.reports.find((report) => report.kind === kind);
      return current?.runId !== previousId ? current?.status : null;
    }, { timeout: 20000 }).toBe("completed");
    const report = bundle.reports.find((item) => item.kind === kind);
    if (kind === "terminal" && screenshotPath) await page.screenshot({ path: screenshotPath.replace(/\.png$/, "-terminal.png"), animations: "disabled" });
    assert.deepEqual(Object.keys(report).sort(), ["checks", "cleanup", "finishedAt", "kind", "probeFile", "runId", "startedAt", "status", "targetIdentityVerified"].sort());
    assert.equal(report.targetIdentityVerified, false);
    runSentinels.push(report.runId, ...[report.probeFile].filter(Boolean));
    assert.equal(report.checks.length, ["channels", "tcp"].includes(kind) ? 1 : 2);
    for (const check of report.checks) {
      assert.deepEqual(Object.keys(check).sort(), ["actualExitCode", "code", "expectedExitCode", "id", "status"].sort());
      assert.equal(check.status, variant === "tcp-denied" ? "unsupported" : "pass", `${kind}/${check.id}: ${check.code}`);
      assert.equal(check.code, variant === "tcp-denied" ? "SSH_TUNNEL_FORWARD_PROHIBITED" : "OK");
    }
    if (kind === "exec" || kind === "terminal") {
      assert.deepEqual(report.checks.map((item) => [item.id, item.expectedExitCode, item.actualExitCode]),
        [[`${kind}-success`, 0, 0], [`${kind}-nonzero`, 23, 23]]);
    } else if (kind === "channels") {
      assert.equal(report.checks[0].id, "concurrent-channels");
    } else if (kind === "tcp") {
      assert.equal(report.checks[0].id, "tcp-forward");
      assert.equal(report.probeFile, null);
    } else {
      assert.equal(report.cleanup, "removed");
      assert.match(report.probeFile, /^\/tmp\/unfour-a4-[0-9a-f-]{36}$/);
      assert.deepEqual(report.checks.map((item) => item.id), ["sftp-roundtrip", "sftp-cleanup"]);
    }
    assert.equal(received.filter((line) => line === "FIXTURE_AUTH_OK").length, authCount,
      "a capability check must reuse the authenticated transport");
  }
  assert.deepEqual(Object.keys(bundle).sort(), ["app", "manualChecks", "reports", "schemaVersion", "scope"].sort());
  assert.deepEqual(bundle.app, { version: metadata.version, commit: metadata.commit, channel: metadata.channel, distribution: metadata.distribution });
  assert.equal(bundle.schemaVersion, 1);
  assert.equal(bundle.scope, "temporary-ssh");
  assert.equal(bundle.reports.length, 5);
  assert.deepEqual(bundle.manualChecks, ["second-login", "expiry", "standalone-file-transfer", "helper-exit", "controlled-disconnect", "task-flow-composition"]);
  const reportText = await dialog.getByTestId("temporary-capability-report").textContent();
  for (const sentinel of [...sentinels, "tcp.fixture.invalid", "denied.fixture.invalid", "32123", "UNFOUR_A4_STDOUT", "UNFOUR_A4_STDERR", "unfour-a4-sftp-probe"]) {
    assert.ok(!reportText.includes(sentinel), "report must not contain launch material or raw remote output");
  }
  if (screenshotPath) await page.screenshot({ path: screenshotPath });
  await verifyCopiedReport(page, dialog, reportText);
  await dialog.getByRole("button", { name: /^(Close|关闭)$/, exact: true }).last().click();
  await expect(dialog).toHaveCount(0);
  const markerCount = received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length;
  await page.locator(".xterm-helper-textarea:visible").last().focus();
  await page.keyboard.type("check");
  await page.keyboard.press("Enter");
  await expect.poll(() => received.filter((line) => line === "FIXTURE_ROUNDTRIP_OK").length).toBe(markerCount + 1);
  console.log("Manual exec 0/23, concurrent channels, SFTP cleanup and TCP success/denial passed; report is sanitized and original PTY still replies");
  return { reportText, sentinels: runSentinels };
}

async function verifyCopiedReport(page, dialog, reportText) {
  await page.evaluate(() => {
    window.__a4ReportCopy = { writeText: navigator.clipboard.writeText, values: [] };
    navigator.clipboard.writeText = async (value) => { window.__a4ReportCopy.values.push(value); };
  });
  try {
    await dialog.getByRole("button", { name: /^(Copy sanitized report|复制脱敏报告)$/ }).click();
    await expect.poll(() => page.evaluate(() => window.__a4ReportCopy.values.length)).toBe(1);
    assert.equal(await page.evaluate(() => window.__a4ReportCopy.values[0]), reportText);
  } finally {
    await page.evaluate(() => {
      navigator.clipboard.writeText = window.__a4ReportCopy.writeText;
      delete window.__a4ReportCopy;
    });
  }
}

export async function verifyDisconnectedReport(page, received, previousReportText) {
  const authCount = received.filter((line) => line === "FIXTURE_AUTH_OK").length;
  await page.locator(".xterm-helper-textarea:visible").last().focus();
  await page.keyboard.type("quit");
  await page.keyboard.press("Enter");
  await expect.poll(() => received.includes("FIXTURE_PTY_CLOSED")).toBe(true);
  await page.getByRole("button", { name: /^(Capability checks|能力检测)$/ }).click();
  const dialog = page.getByRole("dialog", { name: /^(Capability checks|能力检测)$/ });
  await expect(dialog.getByRole("combobox")).toBeDisabled();
  await expect(dialog.getByRole("checkbox").first()).toBeDisabled();
  await expect(dialog.getByRole("button", { name: /^(Run check|开始检测)$/ })).toBeDisabled();
  await expect.poll(() => dialog.getByTestId("temporary-capability-report").textContent()).toBe(previousReportText);
  await verifyCopiedReport(page, dialog, previousReportText);
  assert.equal(received.filter((line) => line === "FIXTURE_AUTH_OK").length, authCount);
  await dialog.getByRole("button", { name: /^(Close|关闭)$/, exact: true }).last().click();
  console.log("Disconnected temporary session retains its report for copying and cannot run another probe");
}
