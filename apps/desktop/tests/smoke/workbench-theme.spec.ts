import { expect, test, type Locator, type Page } from "@playwright/test";
import {
  openApiScene, openDatabaseScene, openFlowScene, openSshScene,
  prepareWorkbenchScene, type Appearance,
} from "./workbench-fixtures";

test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });

async function resolvedColor(page: Page, token: string) {
  return page.evaluate((name) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  }, token);
}

async function expectEditorCanvas(page: Page) {
  await expect(page.locator(".monaco-editor:visible")).toHaveCSS("background-color", await resolvedColor(page, "--u-color-canvas"));
}

async function expectEditorFocus(page: Page) {
  const editor = page.locator(".monaco-editor:visible");
  await editor.click({ position: { x: 120, y: 22 } });
  await page.keyboard.press("Control+Home");
  const focus = await resolvedColor(page, "--u-color-focus");
  await expect(editor.locator(".cursor").first()).toHaveCSS("background-color", focus);
  await expect(editor.locator(".bracket-match").first()).toHaveCSS("border-top-color", focus);
  await expectEditorCanvas(page);
}

async function changeTheme(page: Page, theme: Appearance) {
  await page.locator(".app-shell > header").getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("dialog").getByRole("combobox", { name: "Theme", exact: true }).selectOption(theme);
  await page.keyboard.press("Escape");
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
}

async function expectReadable(controls: Record<string, Locator>) {
  for (const [name, control] of Object.entries(controls)) {
    await expect(control).toBeVisible();
    const ratio = await control.evaluate((element) => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 1;
      const context = canvas.getContext("2d", { willReadFrequently: true })!;
      const rgba = (color: string) => {
        context.clearRect(0, 0, 1, 1);
        context.fillStyle = color;
        context.fillRect(0, 0, 1, 1);
        const pixel = context.getImageData(0, 0, 1, 1).data;
        return [pixel[0], pixel[1], pixel[2], pixel[3] / 255];
      };
      const composite = (front: number[], back: number[]) =>
        front.slice(0, 3).map((value, i) => value * front[3] + back[i] * (1 - front[3])).concat(1);
      const ancestors: Element[] = [];
      for (let node: Element | null = element; node; node = node.parentElement) ancestors.push(node);
      const background = ancestors.reverse().reduce((back, node) => composite(rgba(getComputedStyle(node).backgroundColor), back), [255, 255, 255, 1]);
      const foreground = composite(rgba(getComputedStyle(element).color), background);
      const luminance = (value: number[]) => value.slice(0, 3).map((channel) => {
        const normalized = channel / 255;
        return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
      }).reduce((sum, channel, i) => sum + channel * [0.2126, 0.7152, 0.0722][i], 0);
      const a = luminance(foreground), b = luminance(background);
      return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    });
    expect(ratio, `${name} text contrast`).toBeGreaterThanOrEqual(4.5);
  }
}

for (const theme of ["light", "dark"] as const) {
  test(`${theme}: default workbench executes requests and queries, then preserves mounted editors across theme changes`, async ({ page }, testInfo) => {
    test.setTimeout(90_000);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await prepareWorkbenchScene(page, theme);
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    const navigate = (name: string) => page.getByRole("navigation", { name: "Modules" }).getByRole("button", { name, exact: true }).click();
    const capture = async (scene: string) => {
      // Monaco keyboard navigation can move overflow-hidden ancestors. Restore
      // the shell's capture origin without changing feature scroll positions.
      await page.evaluate(async () => {
        document.scrollingElement?.scrollTo(0, 0);
        document.querySelectorAll<HTMLElement>(".app-shell, .app-shell > header, .app-shell > header *").forEach((node) => {
          if (node.scrollTop || node.scrollLeft) node.scrollTo(0, 0);
        });
        await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
      });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      const titlebar = page.locator(".app-shell > header");
      await expect(titlebar).toBeInViewport({ ratio: 1 });
      const titlebarBounds = await titlebar.boundingBox();
      expect(titlebarBounds).not.toBeNull();
      for (const control of [titlebar.getByRole("button", { name: /Atlas Development/ }), titlebar.getByRole("button", { name: "Settings", exact: true })]) {
        await expect(control).toBeInViewport({ ratio: 1 });
        const bounds = await control.boundingBox();
        expect(bounds!.x).toBeGreaterThanOrEqual(titlebarBounds!.x);
        expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(titlebarBounds!.x + titlebarBounds!.width);
        expect(bounds!.y).toBeGreaterThanOrEqual(titlebarBounds!.y);
        expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(titlebarBounds!.y + titlebarBounds!.height);
      }
      await page.screenshot({ path: testInfo.outputPath(`${scene}-${theme}.png`), animations: "disabled", caret: "hide" });
    };

    // These scenes dispatch the actual page actions through disposable browser
    // adapters. They do not replace feature components with static screenshots.
    await openApiScene(page);
    const apiEditor = await page.locator(".monaco-editor:visible").elementHandle();
    await expectEditorCanvas(page);
    await expect(page.locator(".monaco-editor:visible")).toContainText("Avery Chen");
    await expectReadable({
      "Request URL": page.getByRole("textbox", { name: "Request URL", exact: true }),
      Collections: page.getByText("Collections", { exact: true }),
      Send: page.getByRole("button", { name: "Send", exact: true }).first(),
      "HTTP status": page.getByText("200 OK", { exact: true }),
    });
    await capture("api");

    await openDatabaseScene(page);
    await expect(page.locator(".u-tabbar").filter({
      has: page.getByRole("tab", { name: "Results", exact: true }),
    })).toHaveCSS("height", "36px");
    const sqlEditor = await page.locator(".monaco-editor:visible").elementHandle();
    await expectEditorCanvas(page);
    const selected = page.getByRole("cell", { name: "Avery Chen", exact: true }).getByRole("button");
    await expect(selected).toBeFocused();
    await expectReadable({
      "Selected result": selected,
      "Column type": page.getByRole("columnheader", { name: /^name TEXT/ }).getByText("TEXT", { exact: true }),
      "Run All": page.getByRole("button", { name: "Run All", exact: true }),
      "Query status": page.getByText("OK", { exact: true }),
    });
    await capture("database");

    await openSshScene(page);
    await expectReadable({ "Terminal tab": page.getByRole("tab", { name: /developer@dev.example.test/ }) });
    await capture("ssh");
    await openFlowScene(page);
    await expectReadable({
      "Flow name": page.getByRole("textbox", { name: "Flow name", exact: true }),
      "Flow run": page.getByRole("button", { name: "Run", exact: true }),
      "Canvas help": page.locator(".flow-canvas-help"),
    });
    await capture("flow");

    await navigate("API Client");
    await expectEditorFocus(page);
    const switched: Appearance = theme === "light" ? "dark" : "light";
    await changeTheme(page, switched);
    expect(await apiEditor?.evaluate((element) => element.isConnected)).toBe(true);
    await expectEditorFocus(page);
    await navigate("Database");
    expect(await sqlEditor?.evaluate((element) => element.isConnected)).toBe(true);
    await expectEditorCanvas(page);
    await expect(page.getByRole("cell", { name: "Jordan Park", exact: true })).toBeVisible();

    await changeTheme(page, theme);
    await expectEditorCanvas(page);
    await navigate("API Client");
    await expectEditorFocus(page);
    expect(await apiEditor?.evaluate((element) => element.isConnected)).toBe(true);
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    expect(await page.evaluate(() => localStorage.getItem("unfour.theme"))).toBe(theme);
    await expect(page.getByRole("textbox", { name: "Request URL", exact: true })).toBeVisible();
    expect(errors).toEqual([]);
  });
}
