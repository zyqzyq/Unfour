// Actual-EXE checks using only public UI actions and a fixed synthetic SSH fixture.
import assert from "node:assert/strict";
import { measureCurlClick } from "./measure-curl-click.mjs";
import { expect } from "@playwright/test";

const markCount = (received, marker) => received.filter((line) => line === marker).length;
const screenshot = (path, suffix) => path?.replace(/\.png$/, `-${suffix}.png`);
const recoveryNotices = (page) => page.locator("p:visible").filter({
  hasText: /^(The HTTP response was received, but terminal|Terminal restoration could not be confirmed|已收到 HTTP 响应，但无法确认终端恢复|无法确认终端已恢复)/,
}).allTextContents();

export async function verifyCurlCapability(page, received, screenshotPath) {
  const auths = markCount(received, "FIXTURE_AUTH_OK");
  const sentinels = [];
  await page.getByRole("button", { name: /^(Capability checks|能力检测)$/ }).click();
  const dialog = page.getByRole("dialog", { name: /^(Capability checks|能力检测)$/ });
  const run = dialog.getByRole("button", { name: /^(Run check|开始检测)$/ });
  let reportText;
  for (const kind of ["transport", "curl"]) {
    const transportBefore = markCount(received, "FIXTURE_TRANSPORT_PROBE");
    const curlBefore = markCount(received, "FIXTURE_CURL_PROBE");
    const restoredBefore = markCount(received, "FIXTURE_RAW_RESTORED");
    await dialog.getByRole("combobox", { name: /^(Check to run|检测项目)$/ }).selectOption(kind);
    await dialog.getByRole("checkbox", { name: /authorized test asset|已获授权的测试资产/ }).check();
    await expect(run).toBeDisabled();
    await dialog.getByRole("checkbox", { name: /idle POSIX shell|空闲.*POSIX|POSIX.*空闲/ }).check();
    const reportView = dialog.getByTestId("temporary-capability-report");
    const previousRun = await reportView.count()
      ? JSON.parse(await reportView.textContent()).reports.find((entry) => entry.kind === kind)?.runId : null;
    await run.click();
    let report;
    await expect.poll(async () => {
      reportText = await dialog.getByTestId("temporary-capability-report").textContent().catch(() => null);
      report = reportText ? JSON.parse(reportText).reports.find((entry) => entry.kind === kind) : null;
      return report?.runId !== previousRun ? report?.status : null;
    }, { timeout: 25000 }).toBe("completed");
    const expected = [
      { id: "transport-roundtrip", code: "OK", status: "pass", expectedExitCode: 23, actualExitCode: 23 },
      { id: "transport-recovery", code: "OK", status: "pass", expectedExitCode: null, actualExitCode: null },
      ...(kind === "curl" ? [
        { id: "curl-tools", code: "OK", status: "pass", expectedExitCode: 0, actualExitCode: 0 },
        { id: "curl-recovery", code: "OK", status: "pass", expectedExitCode: null, actualExitCode: null },
      ] : []),
    ];
    assert.deepEqual(report.checks, expected);
    assert.equal(report.targetIdentityVerified, false);
    assert.equal(markCount(received, "FIXTURE_TRANSPORT_PROBE"), transportBefore + 1);
    assert.equal(markCount(received, "FIXTURE_CURL_PROBE"), curlBefore + (kind === "curl" ? 1 : 0));
    assert.equal(markCount(received, "FIXTURE_RAW_RESTORED"), restoredBefore + (kind === "curl" ? 2 : 1));
    assert.equal(markCount(received, "FIXTURE_AUTH_OK"), auths);
    for (const raw of ["UNFOUR_TRANSPORT_ROUNDTRIP", "UNFOUR_CURL_TOOLS", "http://127.0.0.1:1/"]) {
      assert.ok(!reportText.includes(raw), "report must exclude raw probe data and destinations");
    }
    sentinels.push(report.runId);
    await expect(run).toBeDisabled();
    if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, `${kind}-capability`), animations: "disabled" });
  }
  await dialog.getByRole("button", { name: /^(Close|关闭)$/ }).last().click();
  console.log("Independent terminal transport and transport-gated curl checks passed, with exact exit/recovery results and no reauthentication");
  return { reportText, sentinels };
}

async function copiedResponse(page) {
  // Intercept only this click's synthetic clipboard write, never the OS clipboard.
  await page.evaluate(() => {
    window.__curlQaCopy = { original: navigator.clipboard.writeText, values: [] };
    navigator.clipboard.writeText = async (value) => { window.__curlQaCopy.values.push(value); };
  });
  try {
    await page.getByRole("button", { name: /^(Copy response body|复制响应正文)$/ }).click();
    await expect.poll(() => page.evaluate(() => window.__curlQaCopy.values.length)).toBe(1);
    return await page.evaluate(() => window.__curlQaCopy.values[0]);
  } finally {
    await page.evaluate(() => { navigator.clipboard.writeText = window.__curlQaCopy.original; delete window.__curlQaCopy; });
  }
}

export async function verifyTerminalCurlApi(page, received, terminalLabel, screenshotPath) {
  const secret = "UNFOUR_CURL_QA_TOKEN_9f4c9e40";
  const body = JSON.stringify({ input: '北京 "双引号"\n第二行', model: "fixture-embedding", encoding_format: "float" });
  const auths = markCount(received, "FIXTURE_AUTH_OK");
  await page.getByRole("button", { name: /^(API Client|API 客户端|API)$/i }).first().click();
  await page.getByRole("button", { name: /^(New Request|新建请求)$/ }).first().click();
  const url = page.getByRole("textbox", { name: /^(Request URL|请求 URL)$/ });
  await url.fill("http://terminal.fixture.invalid/json");
  await page.getByRole("combobox", { name: /^(HTTP method|HTTP 方法)$/ }).selectOption("POST");
  await page.getByRole("button", { name: /^(Auth|认证)(?:\s|$)/ }).first().click();
  await page.getByRole("combobox", { name: "Auth type", exact: true }).selectOption("bearer");
  await page.getByLabel("Token", { exact: true }).fill(secret);
  await page.getByRole("button", { name: /^(Body|正文)(?:\s|$)/ }).first().click();
  await page.getByRole("button", { name: "raw", exact: true }).first().click();
  await page.getByRole("combobox", { name: "Raw body type", exact: true }).selectOption("json");
  // Monaco uses native EditContext on recent WebView2, and a textarea on
  // older engines. Both expose the same textbox role for keyboard input.
  const editor = page.locator(".monaco-editor:visible").getByRole("textbox").first();
  try {
    await editor.focus();
    console.log("Request editor input:", await editor.getAttribute("class"));
  } catch (error) {
    console.log("Request editor controls:", JSON.stringify(await page.locator(".monaco-editor").evaluateAll((editors) => editors.map((item) => ({
      className: item.className,
      visible: item.getBoundingClientRect().height > 0,
      controls: Array.from(item.querySelectorAll("textarea, [contenteditable], [role=textbox]")).map((control) => ({
        tag: control.tagName, className: control.className, role: control.getAttribute("role"), contenteditable: control.getAttribute("contenteditable"),
      })),
    })))));
    if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, "editor-failure"), animations: "disabled" });
    throw error;
  }
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.insertText(body);
  await page.getByRole("button", { name: /^(Network route|网络路由) ·/ }).click();
  const route = page.getByRole("dialog", { name: /^(API network route|API 网络路由)$/ });
  const targets = route.getByRole("combobox", { name: /^(This request|当前请求)$/ });
  await expect.poll(() => targets.locator('option[value^="live:"]').count()).toBe(1);
  const target = targets.locator('option[value^="live:"]').first();
  assert.ok((await target.textContent()).includes(terminalLabel));
  await targets.selectOption(await target.getAttribute("value"));
  await expect(route.getByRole("combobox", { name: /^(Execution mode|执行方式)$/ })).toHaveValue("tcp");
  await route.getByRole("combobox", { name: /^(Execution mode|执行方式)$/ }).selectOption("terminalCurl");
  if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, "route"), animations: "disabled" });
  await route.getByRole("button", { name: /^(Apply|应用)$/ }).click();
  const send = page.getByRole("button", { name: /^(Send|发送)$/ }).first();
  const confirm = page.getByRole("checkbox", { name: /For this send|本次发送前/ });
  if (process.env.UNFOUR_BASTION_QA_CLICK_TIMING) {
    return measureCurlClick(page, { url, send, confirm, received });
  }
  const cases = [
    { path: "json", marker: "FIXTURE_CURL_RESPONSE 200", status: 200 },
    { path: "error", marker: "FIXTURE_CURL_RESPONSE 500", status: 500 },
    { path: "redirect", marker: "FIXTURE_CURL_RESPONSE 302", status: 302 },
    { path: "connect-failed", marker: "FIXTURE_CURL_CONNECT_FAILED" },
    { path: "wait", marker: "FIXTURE_CURL_WAIT" },
    ...(process.env.UNFOUR_BASTION_QA_RAW_RECOVERY ? [{ path: "timeout", marker: "FIXTURE_CURL_WAIT" }] : []),
  ];
  for (const item of cases) {
    await url.fill(`http://terminal.fixture.invalid/${item.path}`);
    await expect(confirm).not.toBeChecked();
    await expect(send).toBeDisabled();
    await expect(confirm).toBeEnabled();
    await confirm.check();
    const before = markCount(received, item.marker);
    const restored = markCount(received, "FIXTURE_RAW_RESTORED");
    await send.click();
    try {
      await expect.poll(() => markCount(received, item.marker), { timeout: 25000 }).toBe(before + 1);
    } catch (error) {
      console.log("Terminal curl dispatch markers:", item.path, received.filter((line) => /^FIXTURE_(CURL|RAW)_/.test(line)));
      console.log("Terminal curl recovery notice:", await recoveryNotices(page));
      if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, `${item.path}-dispatch-failure`), animations: "disabled" });
      throw error;
    }
    await expect(confirm).not.toBeChecked();
    if (item.status) {
      await expect(page.getByText(new RegExp(`^${item.status}(?:\\s|$)`)).first()).toBeVisible();
      const output = await copiedResponse(page);
      if (item.path === "json") {
        assert.ok(Buffer.byteLength(output) > 64 * 1024, "API response must survive the old Task 64 KiB boundary");
        assert.equal(typeof JSON.parse(output), "object");
        await page.getByRole("button", { name: /^(Timing|耗时|时间)$/ }).click();
        await expect(page.getByText(/^(curl HTTP transfer|curl HTTP 总耗时)$/)).toBeVisible();
        await expect(page.getByText("195 ms", { exact: true })).toBeVisible();
        await expect(page.getByText("120 ms", { exact: true })).toBeVisible();
        if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, "timing"), animations: "disabled" });
        await page.getByRole("button", { name: /^(Body|正文)$/ }).last().click();
        if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, "success"), animations: "disabled" });
      }
      if (item.path === "error") assert.ok(output.length > 0, "HTTP 500 must preserve the server response body");
      if (item.path === "redirect") await expect(page.getByText(/without automatically following|不会自动跟随跳转/)).toBeVisible();
    } else if (item.path === "connect-failed") {
      await expect(page.getByText(/target host could not connect to the API|目标主机无法连接 API/)).toBeVisible();
    } else if (item.path === "timeout") {
      // No manual stop: the configured-unlimited desktop request must acquire
      // the terminal route's finite default and finish without another login.
      await expect(page.getByRole("button", { name: /^(Stop|停止|Stopping|停止中)$/ })).toHaveCount(0, { timeout: 45000 });
      await expect(page.getByText(/Execution started, but a complete result|已确认开始执行，但未在时限内收到完整结果/)).toBeVisible();
      if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, "timeout"), animations: "disabled" });
    } else {
      // One PTY is shared with command tasks and capability checks. Manual input
      // must stay blocked while curl owns it, including when API is hidden.
      await page.getByRole("button", { name: /^(SSH Terminal|SSH 终端|SSH)$/i }).first().click();
      await page.getByRole("tab", { name: "Connections", exact: true }).click();
      await page.getByRole("tab", { name: terminalLabel, exact: true }).click();
      await expect(page.getByText(/Terminal in use|终端.*占用/)).toBeVisible();
      const checks = markCount(received, "FIXTURE_ROUNDTRIP_OK");
      await page.locator(".xterm-helper-textarea:visible").last().focus();
      await page.keyboard.type("check"); await page.keyboard.press("Enter");
      await page.waitForTimeout(250);
      assert.equal(markCount(received, "FIXTURE_ROUNDTRIP_OK"), checks);
      await page.getByRole("button", { name: /^(API Client|API 客户端|API)$/i }).first().click();
      await page.getByRole("button", { name: /^(Stop|停止)$/ }).click();
      await expect(page.getByRole("button", { name: /^(Stop|停止|Stopping|停止中)$/ })).toHaveCount(0);
      await expect(page.getByText(/cancelled|已取消/).first()).toBeVisible();
      if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, "cancel"), animations: "disabled" });
    }
    try {
      await expect.poll(() => markCount(received, "FIXTURE_RAW_RESTORED"), { timeout: 15000 }).toBe(restored + 1);
    } catch (error) {
      console.log("Terminal curl restore markers:", item.path, received.filter((line) => /^FIXTURE_(CURL|RAW)_/.test(line)));
      console.log("Terminal curl recovery notice:", await recoveryNotices(page));
      if (screenshotPath) await page.screenshot({ path: screenshot(screenshotPath, `${item.path}-restore-failure`), animations: "disabled" });
      throw error;
    }
    await expect(send).toBeDisabled(); // A result never authorizes another send.
    assert.equal(markCount(received, "FIXTURE_AUTH_OK"), auths);
    console.log(`Terminal curl ${item.path} passed with terminal restoration`);
  }
  await page.getByRole("button", { name: /^(SSH Terminal|SSH 终端|SSH)$/i }).first().click();
  await page.getByRole("tab", { name: "Connections", exact: true }).click();
  await page.getByRole("tab", { name: terminalLabel, exact: true }).click();
  // The backend is restored before the API result arrives. Let the terminal's
  // polling view observe that state before typing; input while its busy notice
  // is visible is intentionally discarded and must never be replayed later.
  await expect(page.getByRole("status").filter({ hasText: /^(Terminal in use|终端正被)/ })).toHaveCount(0);
  const rows = page.locator(".xterm-rows:visible").last();
  await expect(rows).not.toContainText(secret);
  await expect(rows).not.toContainText("fixture-embedding");
  await expect(rows).not.toContainText("terminal.fixture.invalid");
  await expect(rows).not.toContainText(/\\0[0-7]{3}/); // Internal encoded ACK contents must remain hidden.
  const checks = markCount(received, "FIXTURE_ROUNDTRIP_OK");
  await page.locator(".xterm-helper-textarea:visible").last().focus();
  await page.keyboard.type("check"); await page.keyboard.press("Enter");
  await expect.poll(() => markCount(received, "FIXTURE_ROUNDTRIP_OK")).toBe(checks + 1);
  console.log("Terminal curl JSON >64 KiB, HTTP 500, redirect, connect error, cancellation, input exclusion and recovery passed");
  // Ordinary sanitized API history is allowed. Only the synthetic credential
  // belongs in the parent verifier's no-persistence sentinel scan.
  return { sentinels: [secret], requests: cases.length };
}
