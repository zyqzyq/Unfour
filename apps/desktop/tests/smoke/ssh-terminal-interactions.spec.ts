import { expect, test, type Page } from "@playwright/test";
import { openSshScene, prepareWorkbenchScene } from "./workbench-fixtures";

type SshFixtureInput = { data: string; sessionId: string; workspaceId: string };

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

test("history completion keeps terminal focus and the following input in the same SSH session", async ({ page }) => {
  await prepareWorkbenchScene(page, "dark");
  await page.addInitScript(() => {
    const target = globalThis as typeof globalThis & {
      __sshFixtureEcho?: (input: SshFixtureInput) => string;
      __sshFixtureInputs?: SshFixtureInput[];
    };
    target.__sshFixtureInputs = [];
    // The browser PTY stand-in normally answers every key with a newline, which
    // drops the local shell-prompt context. Echo the bytes and restore a prompt
    // only when the line is submitted, matching what a remote shell prints.
    target.__sshFixtureEcho = (input) => {
      target.__sshFixtureInputs?.push({
        data: input.data,
        sessionId: input.sessionId,
        workspaceId: input.workspaceId,
      });
      return input.data.includes("\r") || input.data.includes("\n") ? "\r\ndev@host:~$ " : input.data;
    };
  });
  await page.route("**/browser-mocks/ssh.ts*", async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const needle = 'data: "Input accepted by SSH PTY stream.\\r\\n",';
    const body = source.replace(
      needle,
      "data: globalThis.__sshFixtureEcho?.(input) ?? \"Input accepted by SSH PTY stream.\\r\\n\",",
    );
    expect(body, "SSH input echo must stay observable in this fixture").not.toBe(source);
    await route.fulfill({
      response,
      body: `${body}
mockStore.sshCommandHistory.push(
  { id: "hist-push", workspaceId: "mock-workspace", connectionId: "workbench-ssh", sessionId: "workbench-session", command: "git push origin main", cwd: null, exitCode: 0, durationMs: 1, redacted: false, executedAt: "2026-11-08T06:19:00.000Z" },
  { id: "hist-status", workspaceId: "mock-workspace", connectionId: "workbench-ssh", sessionId: "workbench-session", command: "git status", cwd: null, exitCode: 0, durationMs: 1, redacted: false, executedAt: "2026-11-08T06:19:30.000Z" },
);`,
    });
  });
  await page.goto("/");
  await openSshScene(page);

  const pane = page.locator(".ssh-terminal-pane:visible").first();
  const input = pane.locator(".xterm-helper-textarea");
  const suggestions = page.getByRole("listbox", { name: "Command history suggestions" });
  await expect(input).toBeFocused();
  await pane.click();
  await expect(input).toBeFocused();
  await expectShellChromeStable(page);

  await page.keyboard.type("gi");
  await expect(suggestions).toBeVisible();
  await expect(suggestions.getByRole("option")).toHaveText(["git status", "git push origin main"]);
  await expect(input).toBeFocused();

  await page.keyboard.press("ArrowDown");
  await expect(suggestions.getByRole("option").nth(1)).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowUp");
  await expect(suggestions.getByRole("option").first()).toHaveAttribute("aria-selected", "true");
  await expect(input).toBeFocused();
  await expectShellChromeStable(page);

  await page.keyboard.press("Tab");
  await expect(suggestions).toHaveCount(0);
  await expect(input).toBeFocused();
  await expectShellChromeStable(page);
  await expect.poll(() => sessionInput(page)).toBe("git status");

  await page.keyboard.press("Space");
  await page.keyboard.type("--short");
  await page.keyboard.press("Enter");
  await expect(input).toBeFocused();
  await expect.poll(() => sessionInput(page)).toBe("git status --short\r");
  await expectShellChromeStable(page);

  await page.keyboard.type("zz");
  await expect(suggestions).toHaveCount(0);
  await page.keyboard.press("Tab");
  await expect(input).toBeFocused();
  await expect.poll(() => sessionInput(page)).toBe("git status --short\rzz\t");
  await expectShellChromeStable(page);

  await page.keyboard.press("Enter");
  await page.keyboard.type("gi");
  await expect(suggestions).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(suggestions).toHaveCount(0);
  await expect(input).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(input).toBeFocused();
  await expect.poll(() => sessionInput(page)).toBe("git status --short\rzz\t\rgi\u001b[B");
  await expectShellChromeStable(page);

  const sidebar = page.getByRole("complementary");
  const connections = sidebar.getByRole("tab", { name: "Connections", exact: true });
  await connections.click();
  await expect(connections).toBeFocused();
  await expect(input).not.toBeFocused();
  await page.keyboard.press("Control+f");
  await expect(page.getByRole("textbox", { name: "Search terminal output", exact: true })).toBeFocused();
  await pane.click();
  await expect(input).toBeFocused();

  const tasks = sidebar.getByRole("tab", { name: "Tasks", exact: true });
  await tasks.click();
  await expect(tasks).toHaveAttribute("aria-selected", "true");
  await connections.click();
  await expect(connections).toHaveAttribute("aria-selected", "true");
  const collapseSidebar = page.getByRole("button", { name: "Collapse sidebar", exact: true });
  await collapseSidebar.click();
  const expandSidebar = page.getByRole("button", { name: "Expand sidebar", exact: true });
  await expect(expandSidebar).toBeFocused();
  await expect(expandSidebar).toHaveAttribute("aria-pressed", "false");
});

async function sessionInput(page: Page) {
  const inputs = await page.evaluate(() => {
    const recorded = (globalThis as typeof globalThis & { __sshFixtureInputs?: SshFixtureInput[] }).__sshFixtureInputs ?? [];
    return recorded.map((entry) => ({ ...entry }));
  });
  expect(inputs.length).toBeGreaterThan(0);
  expect(new Set(inputs.map((entry) => entry.sessionId))).toEqual(new Set(["workbench-session"]));
  expect(inputs.every((entry) => entry.workspaceId === "mock-workspace")).toBe(true);
  return inputs.map((entry) => entry.data).join("");
}

async function expectShellChromeStable(page: Page) {
  const sidebar = page.getByRole("complementary");
  await expect(sidebar.getByRole("tab", { name: "Connections", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(sidebar.getByRole("tab", { name: "Tasks", exact: true })).toHaveAttribute("aria-selected", "false");
  await expect(page.getByRole("button", { name: "Collapse sidebar", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(
    page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name: "SSH Terminal", exact: true }),
  ).toHaveAttribute("aria-expanded", "true");
}
