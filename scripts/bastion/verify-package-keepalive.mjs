// Verify the production 60-second heartbeat with the terminal view hidden.
// A synthetic server expires after 75 seconds without SSH traffic and rekeys
// after 10 seconds. No input is sent during the 130-second observation window.
import assert from "node:assert/strict";
import { expect } from "@playwright/test";

const count = (received, marker) => received.filter((line) => line.startsWith(marker)).length;

export async function verifyBackgroundKeepalive(page, received, terminalLabel, screenshotPath) {
  assert.ok(received.includes("FIXTURE_KEEPALIVE_QA idle=75 rekey=10"));
  await page.getByRole("button", { name: /^(API Client|API 客户端|API)$/i }).first().click();
  await page.waitForTimeout(500); // Settle the view switch before observing traffic.
  const input = count(received, "FIXTURE_PTY_DATA ");
  const resizes = count(received, "FIXTURE_RESIZE ");
  const auths = count(received, "FIXTURE_AUTH_OK");
  const shells = count(received, "FIXTURE_PTY_OK");
  const closed = count(received, "FIXTURE_PTY_CLOSED");
  const start = Date.now();
  console.log("Observing 130s without terminal input; server idle cutoff=75s, rekey=10s");
  while (Date.now() - start < 130000) {
    await page.waitForTimeout(Math.min(10000, 130000 - (Date.now() - start)));
    assert.equal(count(received, "FIXTURE_PTY_DATA "), input, "heartbeat must never write PTY input");
    assert.equal(count(received, "FIXTURE_RESIZE "), resizes, "resize traffic must not keep this test alive");
    assert.equal(count(received, "FIXTURE_AUTH_OK"), auths, "idle recovery must not reauthenticate");
    assert.equal(count(received, "FIXTURE_PTY_OK"), shells, "idle recovery must not replace the shell");
    assert.equal(count(received, "FIXTURE_PTY_CLOSED"), closed);
  }
  await page.getByRole("button", { name: /^(SSH Terminal|SSH 终端|SSH)$/i }).first().click();
  await page.getByRole("tab", { name: "Connections", exact: true }).click();
  await page.getByRole("tab", { name: terminalLabel, exact: true }).click();
  await expect(page.getByRole("status").filter({ hasText: /^(Terminal in use|终端正被)/ })).toHaveCount(0);
  const before = count(received, "FIXTURE_ROUNDTRIP_OK");
  await page.locator(".xterm-helper-textarea:visible").last().focus();
  await page.keyboard.type("check");
  await page.keyboard.press("Enter");
  await expect.poll(() => count(received, "FIXTURE_ROUNDTRIP_OK")).toBe(before + 1);
  assert.equal(count(received, "FIXTURE_AUTH_OK"), auths);
  if (screenshotPath) await page.screenshot({ path: screenshotPath, animations: "disabled" });
  console.log("PASS: background keepalive survived 130s idle and accelerated rekey; same login/shell, zero PTY or resize traffic");
}
