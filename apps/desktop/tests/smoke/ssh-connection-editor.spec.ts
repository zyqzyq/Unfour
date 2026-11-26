import { expect, test } from "@playwright/test";

test("keeps the SSH editor open through a slow test and lets the user correct the host", async ({ page }) => {
  let finishTest!: () => void;
  const pending = new Promise<void>((resolve) => { finishTest = resolve; });
  await page.exposeFunction("waitForFailedSshTest", async () => {
    await pending;
    return { ok: false, message: "Fixture host unavailable" };
  });
  await page.addInitScript(() => localStorage.setItem("unfour.locale", "en"));
  await page.route("**/browser-mocks/ssh.ts*", async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const testBranch = 'if (command === "ssh_connection_test") {';
    expect(source).toContain(testBranch);
    const body = source.replace(testBranch, `${testBranch}
      if (args.input.host === "first.example.test") return window.waitForFailedSshTest();`);
    await route.fulfill({ response, body });
  });

  await page.goto("/");
  await page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal" }).click();
  await page.getByRole("complementary").getByRole("button", { name: "New Connection", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "New SSH Connection" });
  const host = editor.getByLabel("Host", { exact: true });
  const testConnection = editor.getByRole("button", { name: "Test connection" });
  await host.fill("first.example.test");
  await editor.getByLabel("Username").fill("fixture");
  await expect(editor.getByRole("radio", { name: "Password", exact: true })).toBeChecked();
  await editor.getByRole("radio", { name: "No auth", exact: true }).click();
  await testConnection.click();
  await expect(host).toBeDisabled();
  await expect(editor.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  await page.keyboard.press("Escape");
  await page.locator(".u-dialog-overlay").click({ position: { x: 2, y: 2 } });
  await expect(editor).toBeVisible();

  finishTest();
  const failure = page.getByRole("dialog", { name: "Connection failed" });
  await expect(failure).toContainText("Fixture host unavailable");
  await failure.getByRole("button", { name: "Close", exact: true }).press("Enter");
  await expect(testConnection).toBeFocused();
  await expect(host).toHaveValue("first.example.test");
  await host.fill("corrected.example.test");
  await testConnection.click();
  await expect(page.getByRole("dialog", { name: "Connection succeeded" })).toContainText("corrected.example.test");
});
