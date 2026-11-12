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

if (process.platform !== "win32" || process.argv.length !== 3) {
  throw new Error("Usage on Windows: node verify-windows-package.mjs <extracted-package-directory>");
}
const directory = resolve(process.argv[2]);
const metadata = JSON.parse(await readFile(join(directory, "build-metadata.json"), "utf8"));
assert.equal(metadata.channel, "test");
const root = await mkdtemp(join(tmpdir(), "unfour-a2-qa-"));
const webview = join(root, "webview");
const workspace = join(root, "workspace");
await mkdir(webview);
// Exercise the interactive flow with external HTTP(S) unavailable to the app.
// The local proxy discards connections without reading URLs, headers or bodies.
const offlineProxy = createServer((socket) => socket.destroy());
offlineProxy.listen(0, "127.0.0.1");
await once(offlineProxy, "listening");
const proxy = "http://127.0.0.1:" + offlineProxy.address().port;
const localHosts = "localhost,127.0.0.1,tauri.localhost";
const env = { ...process.env, UNFOUR_DATA_DIR: workspace, WEBVIEW2_USER_DATA_FOLDER: webview,
  HTTP_PROXY: proxy, HTTPS_PROXY: proxy, ALL_PROXY: proxy, NO_PROXY: localHosts,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "--remote-debugging-address=127.0.0.1 --remote-debugging-port=0 --proxy-server=" + proxy + " --proxy-bypass-list=tauri.localhost" };
const children = [];
function launch(name, args) {
  const child = spawn(join(directory, name), args, { cwd: directory, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
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
let browser;
try {
  const fixture = launch("bastion-loopback.exe", []);
  createInterface({ input: fixture.stdout }).on("line", (line) => received.push(line));
  const ready = await waitFor(() => received.find((line) => line.startsWith("FIXTURE_READY ")), "fixture did not start");
  const [, port, fingerprint] = ready.split(" ");
  const url = "ssh://fixture-user:fixture-grant@127.0.0.1:" + port;
  const argv = ["-url", url, "-newtab", label];
  const rejected = launch("unfour.exe", [...argv, "-exec", "SYNTHETIC_EXTRA"]);
  const rejectedOutput = [];
  rejected.stderr.on("data", (bytes) => rejectedOutput.push(bytes));
  const [rejectedCode] = await once(rejected, "exit");
  assert.equal(rejectedCode, 2);
  assert.equal(Buffer.concat(rejectedOutput).toString().trim(), "BASTION_LAUNCH_UNSUPPORTED");
  console.log("Malformed Windows argv rejected before app startup");
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
  for (let i = 0; i < 2; i++) {
    if (i === 1) {
      await page.getByRole("button", { name: /^(API Client|API 客户端|API)$/i }).first().click();
      const warm = launch("unfour.exe", argv);
      await waitFor(() => warm.exitCode !== null, "secondary process did not forward and exit");
      assert.equal(warm.exitCode, 0);
      assert.equal(primary.exitCode, null);
    }
    await expect(page.getByText(fingerprint, { exact: true })).toBeVisible();
    const sidebarSearch = page.getByLabel(/Search SSH connections|搜索 SSH 连接/);
    await expect(sidebarSearch).toBeVisible();
    assert.equal(received.filter((x) => x === "FIXTURE_AUTH_OK").length, i);
    await page.getByRole("button", { name: /Trust this fingerprint once|仅本次信任此指纹/ }).click();
    const rows = page.locator(".xterm-rows:visible").last();
    await expect(rows).toContainText("A2 fixture ready", { timeout: 12000 });
    const input = page.locator(".xterm-helper-textarea:visible").last();
    await input.focus();
    await page.keyboard.type("check");
    await page.keyboard.press("Enter");
    await expect(rows).toContainText("A2_ECHO_OK");
    const text = await page.locator("body").innerText();
    for (const secret of ["fixture-user", "fixture-grant", label, url]) assert.ok(!text.includes(secret));
    await page.getByRole("button", { name: /Close \/ cancel|关闭／取消/ }).last().click();
    await expect(sidebarSearch).toBeVisible();
    console.log("Temporary terminal " + (i + 1) + " confirmed fingerprint, rendered SSH reply and closed");
  }
  for (const marker of ["FIXTURE_AUTH_OK", "FIXTURE_PTY_OK", "FIXTURE_ROUNDTRIP_OK"]) {
    assert.equal(received.filter((x) => x === marker).length, 2);
  }
  await page.evaluate(() => window.__TAURI_INTERNALS__.invoke("plugin:window|close", { label: "main" })).catch(() => {});
  await waitFor(() => primary.exitCode !== null, "test window did not close");
  const db = new DatabaseSync(join(workspace, "unfour.sqlite"), { readOnly: true });
  try {
    for (const table of ["ssh_connections", "ssh_host_keys", "ssh_terminal_history", "ssh_command_history", "ssh_task_run", "cloud_sync_outbox"]) {
      assert.equal(db.prepare("SELECT COUNT(*) AS n FROM " + table).get().n, 0, table);
    }
    assert.equal(db.prepare("SELECT COUNT(*) AS n FROM activity_events WHERE action LIKE 'ssh.%'").get().n, 0);
  } finally {
    db.close();
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
    for (const secret of ["fixture-user", "fixture-grant", label, url]) {
      for (const encoding of ["utf8", "utf16le"]) assert.ok(!data.includes(Buffer.from(secret, encoding)), "sensitive sentinel found in isolated storage");
    }
  }
  console.log("PASS: two sessions, zero saved SSH/trust/history/task/activity/outbox rows, no sentinels in the disposable DB/logs/WebView profile");
} finally {
  await browser?.close().catch(() => {});
  for (const child of children) if (child.exitCode === null && !child.killed) child.kill();
  offlineProxy.close();
}
