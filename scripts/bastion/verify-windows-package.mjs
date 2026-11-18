// Developer acceptance test for the dedicated offline package. CDP is enabled
// only for this child and a fresh WebView2 profile. All SSH actions use the UI.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, mkdir, readdir, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as delay } from "node:timers/promises";
import { chromium, expect } from "@playwright/test";
import { verifyCapabilityChecks, verifyDisconnectedReport, verifyTerminalUx } from "./verify-package-capabilities.mjs";
import { verifyTemporaryTasks } from "./verify-package-temporary-task.mjs";
import { verifyCurlCapability, verifyTerminalCurlApi } from "./verify-package-curl.mjs";
import { verifyBackgroundKeepalive } from "./verify-package-keepalive.mjs";
import { verifyCurlRecovery } from "./verify-package-curl-recovery.mjs";

const mode = process.argv[3] ?? "xshell";
const checkKeepalive = Boolean(process.env.UNFOUR_BASTION_QA_KEEPALIVE);
const checkCurlRecovery = Boolean(process.env.UNFOUR_BASTION_QA_RAW_RECOVERY);
if (process.platform !== "win32" || ![3, 4].includes(process.argv.length) || !["xshell", "scrt"].includes(mode)) {
  throw new Error("Usage on Windows: node verify-windows-package.mjs <extracted-package-directory> [xshell|scrt]");
}
const directory = resolve(process.argv[2]);
const metadata = JSON.parse(await readFile(process.env.UNFOUR_BASTION_QA_METADATA ?? join(directory, "build-metadata.json"), "utf8"));
assert.equal(metadata.channel, "test");
const root = await mkdtemp(join(tmpdir(), "unfour-a2-qa-"));
const webview = join(root, "webview");
const workspace = join(root, "workspace");
await mkdir(webview);
const diagnosticTemp = join(root, "temp");
await mkdir(diagnosticTemp);
// Exercise the interactive flow with external HTTP(S) unavailable to the app.
// The local proxy discards connections without reading URLs, headers or bodies.
const offlineProxy = createServer((socket) => socket.destroy());
offlineProxy.listen(0, "127.0.0.1");
await once(offlineProxy, "listening");
const proxy = "http://127.0.0.1:" + offlineProxy.address().port;
const localHosts = "localhost,127.0.0.1,tauri.localhost";
const env = { ...process.env, UNFOUR_DATA_DIR: workspace, WEBVIEW2_USER_DATA_FOLDER: webview,
  TEMP: diagnosticTemp, TMP: diagnosticTemp,
  HTTP_PROXY: proxy, HTTPS_PROXY: proxy, ALL_PROXY: proxy, NO_PROXY: localHosts,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--remote-debugging-address=127.0.0.1 --remote-debugging-port=0 --proxy-server=" + proxy + " --proxy-bypass-list=tauri.localhost" };
const children = [];
function launch(name, args) {
  const executable = name === "bastion-loopback.exe" && process.env.UNFOUR_BASTION_QA_FIXTURE ? process.env.UNFOUR_BASTION_QA_FIXTURE : join(directory, name);
  const child = spawn(executable, args, { cwd: directory, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  children.push(child);
  return child;
}
async function waitFor(check, message, timeout = 25000) {
  const deadline = Date.now() + timeout;
  do {
    const result = await check();
    if (result) return result;
    await delay(100);
  } while (Date.now() < deadline);
  throw new Error(message);
}
const label = "SYNTHETIC-ASSET @different.invalid";
const received = [];
const reportSentinels = ["tcp.fixture.invalid", "denied.fixture.invalid"];
let browser;
let createdTasks = 0;
async function rejectWithDiagnostic(argv, expectedCount) {
  const rejected = launch("unfour.exe", argv);
  const rejectedOutput = [];
  rejected.stderr.on("data", (bytes) => rejectedOutput.push(bytes));
  const reports = join(diagnosticTemp, "Unfour-launch-diagnostics");
  const contents = await waitFor(async () => {
    const names = await readdir(reports).catch((error) => {
      if (error.code === "ENOENT") return [];
      throw error;
    });
    if (names.length !== expectedCount) return null;
    // The create-new filename becomes visible before the writer flushes. Wait
    // for every expected argv slot rather than treating file existence as completion.
    const texts = await Promise.all(names.map((name) => readFile(join(reports, name), "utf8")));
    return texts.every((report) => {
      const count = report.match(/^argument_count=(\d+)$/m);
      return count && report.endsWith("\n")
        && (report.match(/^argv\[\d+\] = /gm)?.length ?? 0) === Math.min(Number(count[1]), 64);
    }) ? texts : null;
  }, "rejected launch did not finish writing its redacted report");
  for (const report of contents) {
    assert.match(report, /^Unfour launch diagnostic v1\nBASTION_LAUNCH_UNSUPPORTED\n/);
    assert.match(report, /argv\[0\] = (-url|\/T)/);
    for (const secret of ["fixture-user", "fixture-grant", label, "SYNTHETIC_EXTRA", "SYNTHETIC_CRT_USER", "SYNTHETIC_CRT_GRANT", "gateway.invalid"]) {
      assert.ok(!report.includes(secret), "launch report leaked a synthetic value");
    }
  }
  assert.equal(rejected.exitCode, null, "diagnostic should wait for native dialog dismissal");
  await waitFor(() => Buffer.concat(rejectedOutput).includes(10), "rejected launch did not emit its fixed error code");
  assert.equal(Buffer.concat(rejectedOutput).toString().trim(), "BASTION_LAUNCH_UNSUPPORTED");
  // This unattended check ends only its own rejected child. The native dialog's
  // rendering and OK -> exit 2 path are checked separately on the packaged app.
  const exited = once(rejected, "exit");
  rejected.kill();
  await exited;
}
async function verify() {
  const fixture = launch("bastion-loopback.exe", []);
  if (process.env.UNFOUR_BASTION_QA_CLICK_TIMING) fixture.stderr.pipe(process.stderr);
  createInterface({ input: fixture.stdout }).on("line", (line) => received.push(line));
  const ready = await waitFor(() => received.find((line) => line.startsWith("FIXTURE_READY ")), "fixture did not start");
  const [, port] = ready.split(" ");
  const url = "ssh://fixture-user:fixture-grant@127.0.0.1:" + port;
  const xshellArgv = ["-url", url, "-newtab", label];
  const scrtArgv = ["/T", "/N", label, "/SSH2", "127.0.0.1", "/P", port, "/L", "fixture-user", "/PASSWORD", "fixture-grant"];
  const argvFor = (kind) => kind === "scrt" ? scrtArgv : xshellArgv;
  const argv = argvFor(mode);
  const launches = [mode, mode, mode === "scrt" ? "xshell" : "scrt"];
  await rejectWithDiagnostic([...argv, "-exec", "SYNTHETIC_EXTRA"], 1);
  console.log("Malformed Windows argv produced a redacted diagnostic before app startup");
  const initialSessionStart = received.length;
  const primary = launch("unfour.exe", argv);
  // The ephemeral debugging port is discovered only inside our own child profile.
  const debugPort = await waitFor(async () => {
    assert.equal(primary.exitCode, null, "cold process exited before UI readiness");
    const files = await readdir(webview, { recursive: true });
    const name = files.find((file) => file.endsWith("DevToolsActivePort"));
    if (name) return (await readFile(join(webview, name), "utf8")).split(/\r?\n/)[0];
  }, "test WebView2 profile did not publish its debugging port");
  browser = await chromium.connectOverCDP("http://127.0.0.1:" + debugPort);
  const page = await waitFor(() => browser.contexts().flatMap((c) => c.pages()).find((p) => /^https?:\/\/tauri\.localhost\//.test(p.url())), "packaged application page did not load");
  page.setDefaultTimeout(12000);
  if (process.env.UNFOUR_BASTION_QA_CLICK_TIMING) {
    const terminalLabel = label + " · 127.0.0.1:" + port;
    await expect(page.getByRole("tab", { name: terminalLabel, exact: true })).toBeVisible();
    await expect(page.locator(".xterm-rows:visible").last()).toContainText("A2 fixture ready");
    await verifyTerminalCurlApi(page, received, terminalLabel);
    await page.evaluate(() => window.__TAURI_INTERNALS__.invoke("plugin:window|close", { label: "main" })).catch(() => {});
    await waitFor(() => primary.exitCode !== null, "test window did not close");
    return;
  }
  for (let i = 0; i < launches.length; i++) {
    let sessionStart = initialSessionStart;
    if (i > 0) {
      await page.getByRole("button", { name: /^(API Client|API 客户端|API)$/i }).first().click();
      sessionStart = received.length;
      const warm = launch("unfour.exe", argvFor(launches[i]));
      await waitFor(() => warm.exitCode !== null, "secondary process did not forward and exit");
      assert.equal(warm.exitCode, 0);
      assert.equal(primary.exitCode, null);
    }
    await expect(page.getByRole("tab", { name: label + " · 127.0.0.1:" + port, exact: true })).toBeVisible();
    const sidebarSearch = page.getByLabel(/Search SSH connections|搜索 SSH 连接/);
    await expect(sidebarSearch).toBeVisible();
    await expect(page.getByRole("button", { name: /Trust this fingerprint once|仅本次信任此指纹/ })).toHaveCount(0);
    const rows = page.locator(".xterm-rows:visible").last();
    await expect(rows).toContainText("A2 fixture ready", { timeout: 12000 });
    assert.equal(received.filter((x) => x === "FIXTURE_AUTH_OK").length, i + 1);
    // The server must receive the fitted PTY size without a manual window resize.
    const visibleRows = await rows.locator(":scope > div").count();
    assert.ok(visibleRows > 0);
    const remoteRows = Math.min(100, Math.max(8, visibleRows));
    await waitFor(() => received.slice(sessionStart).some((line) =>
      line.startsWith("FIXTURE_RESIZE ") && Number(line.split(" ")[2]) === remoteRows),
      "new PTY did not receive the displayed row count");
    const input = page.locator(".xterm-helper-textarea:visible").last();
    await input.focus();
    await page.keyboard.type("check");
    await page.keyboard.press("Enter");
    await expect(rows).toContainText("A2_ECHO_OK");
    if (i === 0) {
      await rejectWithDiagnostic([...scrtArgv, "/SCRIPT", "SYNTHETIC_EXTRA"], 2);
      assert.equal(primary.exitCode, null, "rejected hot launch affected the running app");
      await expect(rows).toContainText("A2_ECHO_OK");
      await expect(sidebarSearch).toBeVisible();
      await verifyTerminalUx(page, received);
    }
    const text = await page.locator("body").innerText();
    assert.ok(text.includes(label));
    assert.ok(text.includes("127.0.0.1:" + port));
    for (const secret of ["fixture-user", "fixture-grant", url]) assert.ok(!text.includes(secret));
    const completed = await verifyCapabilityChecks(page, received, metadata,
      ["fixture-user", "fixture-grant", label, url, "127.0.0.1"],
      i === 0 ? process.env.UNFOUR_BASTION_QA_SCREENSHOT : undefined);
    reportSentinels.push(...completed.sentinels);
    if (i === 0) {
      const taskChecks = await verifyTemporaryTasks(page, received, label + " · 127.0.0.1:" + port, process.env.UNFOUR_BASTION_QA_TASK_SCREENSHOT);
      createdTasks += taskChecks.createdTasks;
      reportSentinels.push(...taskChecks.sentinels, ...taskChecks.runIds);
      if (checkCurlRecovery) {
        const recovery = await verifyCurlRecovery(page, received, process.env.UNFOUR_BASTION_QA_CURL_SCREENSHOT);
        reportSentinels.push(...recovery.sentinels);
      }
      const curlProbe = await verifyCurlCapability(page, received, process.env.UNFOUR_BASTION_QA_CURL_SCREENSHOT);
      const curlChecks = await verifyTerminalCurlApi(page, received, label + " · 127.0.0.1:" + port, process.env.UNFOUR_BASTION_QA_CURL_SCREENSHOT);
      reportSentinels.push(...curlProbe.sentinels, ...curlChecks.sentinels);
      if (checkKeepalive) await verifyBackgroundKeepalive(page, received, label + " · 127.0.0.1:" + port, process.env.UNFOUR_BASTION_QA_KEEPALIVE_SCREENSHOT);
    }
    if (i === launches.length - 1) await verifyDisconnectedReport(page, received, completed.reportText);
    await page.getByRole("button", { name: /Close \/ cancel|关闭／取消/ }).last().click();
    await expect(page.getByRole("tab", { name: label + " · 127.0.0.1:" + port, exact: true })).toHaveCount(0);
    await expect(sidebarSearch).toBeVisible();
    console.log(launches[i] + " temporary terminal " + (i + 1) + " verified without a trust prompt, displayed the node, rendered SSH reply and closed");
  }
  for (const marker of ["FIXTURE_AUTH_OK", "FIXTURE_PTY_OK"]) {
    assert.equal(received.filter((x) => x === marker).length, launches.length);
  }
  assert.equal(received.filter((x) => x === "FIXTURE_ROUNDTRIP_OK").length, launches.length * 2 + 3 + Number(checkKeepalive) + (checkCurlRecovery ? 2 : 0));
  assert.equal(received.filter((x) => x === "FIXTURE_EXEC_OK 0").length, launches.length * 3);
  assert.equal(received.filter((x) => x === "FIXTURE_EXEC_OK 23").length, launches.length);
  for (const marker of ["FIXTURE_THREE_CHANNELS_OK", "FIXTURE_SFTP_OK", "FIXTURE_SFTP_CREATE_OK", "FIXTURE_SFTP_WRITE_OK", "FIXTURE_SFTP_READ_OK", "FIXTURE_SFTP_EOF_OK", "FIXTURE_SFTP_CLOSE_OK", "FIXTURE_SFTP_REMOVE_OK"]) {
    assert.equal(received.filter((x) => x === marker).length, launches.length, marker);
  }
  assert.ok(!received.includes("FIXTURE_SFTP_REMAINING"), "synthetic SFTP file was not removed");
  assert.equal(received.filter((line) => line === "FIXTURE_TCP_EMPTY_OK").length, launches.length);
  assert.equal(received.filter((line) => line === "FIXTURE_TCP_DENIED").length, launches.length);
  assert.ok(!received.includes("FIXTURE_TCP_UNEXPECTED_DATA"));
  assert.equal(received.filter((line) => line === "FIXTURE_PTY_CLOSED").length, 1);
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke("plugin:window|close", { label: "main" })).catch(() => {});
  await waitFor(() => primary.exitCode !== null, "test window did not close");
  const db = new DatabaseSync(join(workspace, "unfour.sqlite"), { readOnly: true });
  try {
    for (const table of ["ssh_connections", "ssh_host_keys", "ssh_terminal_history", "ssh_command_history", "ssh_task_run", "cloud_sync_outbox"]) {
      assert.equal(db.prepare("SELECT COUNT(*) AS n FROM " + table).get().n, 0, table);
    }
    assert.equal(db.prepare("SELECT COUNT(*) AS n FROM activity_events WHERE action LIKE 'ssh.%' AND action <> 'ssh.task.save'").get().n, 0);
    assert.equal(db.prepare("SELECT COUNT(*) AS n FROM ssh_task").get().n, createdTasks);
    assert.equal(db.prepare("SELECT COUNT(*) AS n FROM ssh_task_local_binding WHERE default_connection_id IS NOT NULL OR last_used_connection_id IS NOT NULL").get().n, 0);
  } finally {
    db.close();
  }
  const webData = new DatabaseSync(join(webview, "EBWebView", "Default", "Web Data"), { readOnly: true });
  try {
    for (const table of ["autofill", "autofill_edge_field_values"]) {
      if (!webData.prepare("SELECT COUNT(*) AS n FROM sqlite_schema WHERE type = 'table' AND name = ?").get(table).n) continue;
      for (const value of ["tcp.fixture.invalid", "denied.fixture.invalid", "fixture-task-ok", "fixture-task-wait"]) {
        assert.equal(webData.prepare("SELECT COUNT(*) AS n FROM " + table + " WHERE value = ?").get(value).n, 0,
          `WebView ${table} must not remember runtime input ${value}`);
      }
    }
  } finally {
    webData.close();
  }
  // All files in this newly created root belong to the test, including the WebView profile.
  for (const file of await readdir(root, { recursive: true, withFileTypes: true })) {
    if (!file.isFile()) continue;
    // WebView2 removes transient lock files asynchronously after window close.
    const data = await readFile(join(file.parentPath, file.name)).catch((error) => {
      if (error.code === "ENOENT") return null;
      throw error;
    });
    if (data === null) continue;
    for (const secret of ["fixture-user", "fixture-grant", label, url, ...reportSentinels]) {
      for (const encoding of ["utf8", "utf16le"]) assert.ok(!data.includes(Buffer.from(secret, encoding)), `sensitive sentinel found in isolated storage: ${join(file.parentPath, file.name)} (${encoding}; marker ${["fixture-user", "fixture-grant", label, url, ...reportSentinels].indexOf(secret)})`);
    }
  }
  console.log("PASS: " + mode + " cold + warm + cross-client warm, " + createdTasks + " saved definitions; zero SSH/trust/history/task-run/runtime-activity/outbox rows or remembered targets; no runtime sentinels in DB/logs/WebView profile");
}
try {
  await verify();
} finally {
  await browser?.close().catch(() => {});
  for (const child of children) if (child.exitCode === null && !child.killed) child.kill();
  offlineProxy.close();
}
