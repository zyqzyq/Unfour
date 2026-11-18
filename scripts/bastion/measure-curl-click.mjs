// Actual WebView send events. The loopback SSH peer returns a 97-byte fixed
// response after 200 ms. It timestamps receipt of the post-decode Q fence.
// Poll the same read-only native lease state that the terminal view consumes.
import assert from "node:assert/strict";
import { expect } from "@playwright/test";

export async function measureCurlClick(page, { url, send, confirm, received }) {
  await url.fill("http://terminal.fixture.invalid/small");
  const workspaceId = await page.evaluate(async () =>
    (await window.__TAURI_INTERNALS__.invoke("workspace_list")).activeWorkspaceId);
  for (let sample = 0; sample < 3; sample++) {
    await confirm.check();
    const offset = received.length;
    await send.evaluate((element, workspaceId) => {
      element.addEventListener("click", () => {
        const start = Date.now();
        window.__curlResponseView = new Promise((resolve, reject) => {
          let sending = false;
          const observer = new MutationObserver(() => {
            const buttons = Array.from(document.querySelectorAll("button"));
            const pending = buttons.some((button) => /^(Stop|停止)$/.test(button.textContent.trim()));
            if (pending) sending = true;
            else if (sending) {
              observer.disconnect();
              clearTimeout(timer);
              resolve(Date.now() - start);
            }
          });
          const timer = setTimeout(() => {
            observer.disconnect();
            reject(new Error("request did not settle in the response view"));
          }, 10000);
          observer.observe(document.body, { childList: true, subtree: true, characterData: true });
        });
        window.__curlLeaseSample = (async () => {
          let held = false;
          while (Date.now() - start < 10000) {
            const entries = await window.__TAURI_INTERNALS__.invoke("ssh_temporary_list", { workspaceId });
            const entry = entries.find((item) => item.status === "connected");
            if (!entry || entry.terminalRecoveryRequired) throw new Error("fixture terminal unavailable");
            if (entry.terminalTaskBusy) held = true;
            else if (held) return { start, release: Date.now() - start };
            await new Promise((resolve) => setTimeout(resolve, 5));
          }
          throw new Error("did not observe native lease acquisition and release");
        })();
      }, { capture: true, once: true });
    }, workspaceId);
    await send.click();
    const [{ start, release }, responseView] = await page.evaluate(() =>
      Promise.all([window.__curlLeaseSample, window.__curlResponseView]));
    const records = () => received.slice(offset).filter((line) => line.startsWith("FIXTURE_COMPLETE_OBSERVED "));
    await expect.poll(() => records().length).toBe(1);
    const complete = Number(records()[0].split(" ")[1]) - start;
    assert.ok(complete > 0 && release >= complete);
    await expect(page.getByText(/^200(?:\s|$)/).first()).toBeVisible();
    await expect(confirm).not.toBeChecked();
    console.log("CURL_CLICK", JSON.stringify({ sample: sample + 1, complete, release, responseView }));
  }
}

