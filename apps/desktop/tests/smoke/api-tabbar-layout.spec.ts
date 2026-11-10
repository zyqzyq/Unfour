import { expect, test } from "@playwright/test";

for (const locale of ["en", "zh-CN"] as const) {
  test(`${locale}: API tabs stay bounded and edge tooltip remains fully visible`, async ({ page }, testInfo) => {
    const zh = locale === "zh-CN";
    await page.addInitScript(({ locale, theme }) => {
      localStorage.setItem("unfour.locale", locale);
      localStorage.setItem("unfour.theme", theme);
    }, { locale, theme: zh ? "light" : "dark" });
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");

    const tabs = page.getByRole("tablist", { name: zh ? "已打开的 API 请求" : "Open API requests" });
    const add = page.getByRole("button", { name: zh ? "新建请求" : "New Request", exact: true });
    const importButton = page.getByRole("button", { name: zh ? "导入 cURL" : "Import cURL", exact: true });
    const tooltip = importButton.locator("span").last();
    await expect(tabs).toBeVisible();

    for (const width of [1440, 960]) {
      await page.setViewportSize({ width, height: 900 });
      await importButton.hover();
      await expect(tooltip).toBeVisible();
      // A visible DOM node can still be clipped by an overflow-hidden frame.
      const clippedBy = await tooltip.evaluate((node) => {
        const bounds = node.getBoundingClientRect();
        const clipped: string[] = [];
        for (let parent = node.parentElement; parent; parent = parent.parentElement) {
          const style = getComputedStyle(parent);
          const frame = parent.getBoundingClientRect();
          if (/hidden|auto|scroll|clip/.test(style.overflowX) &&
            (bounds.left < frame.left || bounds.right > frame.right)) clipped.push(parent.tagName);
          if (/hidden|auto|scroll|clip/.test(style.overflowY) &&
            (bounds.top < frame.top || bounds.bottom > frame.bottom)) clipped.push(parent.tagName);
        }
        return clipped;
      });
      expect(clippedBy).toEqual([]);
      await page.screenshot({ path: testInfo.outputPath(`api-tabbar-${width}.png`), animations: "disabled" });
    }

    await page.mouse.move(0, 0);
    await add.focus();
    await page.keyboard.press("Tab");
    await expect(importButton).toBeFocused();
    await expect(tooltip).toBeVisible();

    await page.setViewportSize({ width: 1440, height: 900 });
    await add.click();
    await add.click();
    await tabs.getByRole("tab").first().click();
    const inactive = tabs.locator(".u-document-tab").nth(1);
    const close = inactive.locator(".u-document-tab-close");
    await expect(close).toHaveCSS("opacity", "0");
    const widthBeforeHover = (await inactive.boundingBox())!.width;
    await inactive.hover();
    await expect(close).toHaveCSS("opacity", "1");
    expect((await inactive.boundingBox())!.width).toBe(widthBeforeHover);
    await page.mouse.move(0, 0);
    await close.focus();
    await expect(close).toHaveCSS("opacity", "1");
    await tabs.getByRole("tab").first().click();
    await page.mouse.move(0, 0);
    await expect(close).toHaveCSS("opacity", "0");
    const rail = await tabs.locator("..").boundingBox();
    await page.screenshot({
      path: testInfo.outputPath("api-three-tabs.png"),
      clip: { x: rail!.x, y: rail!.y, width: rail!.width, height: 142 },
      animations: "disabled",
    });

    await page.setViewportSize({ width: 960, height: 900 });
    for (let index = 0; index < 8; index++) await add.click();
    expect(await tabs.evaluate(node => node.scrollWidth > node.clientWidth)).toBe(true);
    expect(await tabs.evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
    await expect(add).toBeInViewport({ ratio: 1 });
    await expect(importButton).toBeInViewport({ ratio: 1 });
    const tabWidths = await tabs.getByRole("tab").evaluateAll(nodes =>
      nodes.map(node => node.parentElement!.getBoundingClientRect().width));
    expect(tabWidths.every(width => width >= 128 && width <= 224)).toBe(true);
    const verticalBounds = await tabs.evaluate(node => {
      const bar = node.parentElement!.getBoundingClientRect();
      const list = node.getBoundingClientRect();
      return { barTop: bar.top, barBottom: bar.bottom, listTop: list.top, listBottom: list.bottom };
    });
    expect(verticalBounds.listTop).toBeGreaterThanOrEqual(verticalBounds.barTop);
    expect(verticalBounds.listBottom).toBeLessThanOrEqual(verticalBounds.barBottom + 1);
    await importButton.click();
    await expect(page.getByRole("dialog")).toBeVisible();
  });
}
