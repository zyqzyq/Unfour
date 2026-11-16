import { expect, test } from "@playwright/test";
import { openSshScene, prepareWorkbenchScene } from "./workbench-fixtures";

test("terminal recovers a granted clipboard read and keeps a calmer accessible cursor", async ({ page }) => {
  await prepareWorkbenchScene(page, "dark");
  await page.addInitScript(() => {
    let reads = 0;
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: {
      readText: async () => {
        localStorage.setItem("fixture.clipboardReads", String(++reads));
        if (reads === 1) throw new DOMException("Document is not focused", "NotAllowedError");
        return "clipboard-recovery-fixture";
      },
    } });
    Object.defineProperty(navigator, "permissions", { configurable: true, value: {
      query: async () => ({ state: "granted" }),
    } });
  });
  await page.goto("/");
  await openSshScene(page);
  const pane = page.locator(".ssh-terminal-pane:visible").first();
  await pane.click();
  const cursor = pane.locator(".xterm-cursor.xterm-cursor-blink");
  await expect(cursor).toHaveCSS("animation-duration", "1.4s");
  await pane.click({ button: "right" });
  await page.getByRole("menuitem", { name: /Paste.*Ctrl\+V/ }).click();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("fixture.clipboardReads"))).toBe("2");
  await expect(pane.locator(".xterm-helper-textarea")).toBeFocused();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(pane).toContainText("Input accepted by SSH PTY stream.");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(cursor).toHaveCSS("animation-name", "none");
});
