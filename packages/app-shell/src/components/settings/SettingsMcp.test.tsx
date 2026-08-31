// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { SettingsMcp } from "./SettingsMcp";

const mocks = vi.hoisted(() => ({
  configureMcpClient: vi.fn(),
  getMcpBinaryPath: vi.fn(),
  getMcpClientStatus: vi.fn(),
}));

vi.mock("@unfour/command-client", () => mocks);

function renderSettings(locale: "en" | "zh-CN" = "en") {
  return render(
    <I18nProvider initialLocale={locale} storageKey="test.settings-mcp.locale">
      <SettingsMcp />
    </I18nProvider>,
  );
}

beforeEach(() => {
  mocks.getMcpBinaryPath.mockResolvedValue({
    path: "D:\\Apps\\Unfour\\unfour-mcp.exe",
    found: true,
    buildKind: "release",
  });
  mocks.getMcpClientStatus.mockImplementation((client: "codex" | "cursor") =>
    Promise.resolve({
      client,
      status: client === "codex" ? "notConfigured" : "outdated",
      configPath:
        client === "codex"
          ? "C:\\Users\\test\\.codex\\config.toml"
          : "C:\\Users\\test\\.cursor\\mcp.json",
    }),
  );
  mocks.configureMcpClient.mockImplementation((client: "codex" | "cursor") =>
    Promise.resolve({
      client,
      status: "configured",
      configPath:
        client === "codex"
          ? "C:\\Users\\test\\.codex\\config.toml"
          : "C:\\Users\\test\\.cursor\\mcp.json",
    }),
  );
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("SettingsMcp", () => {
  it("prioritizes one-click client actions and removes in-app setup documentation", async () => {
    renderSettings();

    const configureCodex = await screen.findByRole("button", { name: "Configure Codex" });
    expect(screen.getByRole("button", { name: "Update Cursor configuration" })).toBeEnabled();
    expect(screen.queryByText("Config example")).toBeNull();
    expect(screen.queryByRole("button", { name: "Copy config" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Open MCP docs" })).toBeNull();

    fireEvent.click(configureCodex);

    await waitFor(() => expect(mocks.configureMcpClient).toHaveBeenCalledWith("codex"));
    expect(
      await screen.findByText("Codex configured. Restart Codex to load Unfour MCP."),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Codex configured" })).toBeDisabled();
  });

  it("disables both client actions when the runtime MCP binary is unavailable", async () => {
    mocks.getMcpBinaryPath.mockResolvedValue({
      path: "D:\\Apps\\Unfour\\unfour-mcp.exe",
      found: false,
      buildKind: "release",
    });

    renderSettings();

    expect(await screen.findByText("MCP binary not found")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Configure Codex" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Update Cursor configuration" })).toBeDisabled();
    expect(mocks.configureMcpClient).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Copy example prompt" })).toBeEnabled();
  });

  const locales = [
    {
      locale: "en",
      label: "Example Prompt",
      copy: "Copy example prompt",
      copied: "Copied",
      failed: "Copy failed",
      start: "Investigate a backend issue using the saved resources in this Unfour workspace.",
      command: "Copy command",
    },
    {
      locale: "zh-CN",
      label: "示例提示词",
      copy: "复制示例提示词",
      copied: "已复制",
      failed: "复制失败",
      start: "使用当前 Unfour 工作区中已保存的资源排查一个后端问题。",
      command: "复制命令",
    },
  ] as const;

  it.each(locales)("copies the displayed $locale prompt without running actions", async (copy) => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    const open = vi.spyOn(window, "open");
    const persist = vi.spyOn(Storage.prototype, "setItem");
    renderSettings(copy.locale);
    await waitFor(() => expect(screen.getByRole("button", { name: copy.command })).toBeEnabled());

    expect(screen.getByRole("heading", { name: copy.label })).toBeTruthy();
    const prompt = screen.getByText(copy.start, { exact: false }).textContent;
    expect(writeText).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: copy.copy }));

    expect(await screen.findByRole("button", { name: copy.copied })).toBeTruthy();
    expect(writeText).toHaveBeenCalledExactlyOnceWith(prompt);
    expect(mocks.configureMcpClient).not.toHaveBeenCalled();
    expect(mocks.getMcpBinaryPath).toHaveBeenCalledTimes(1);
    expect(mocks.getMcpClientStatus).toHaveBeenCalledTimes(2);
    expect(fetch).not.toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
    expect(persist).not.toHaveBeenCalled();

    // Copy command keeps its own content and feedback after copying the prompt.
    fireEvent.click(screen.getByRole("button", { name: copy.command }));
    await waitFor(() => expect(writeText).toHaveBeenLastCalledWith("D:\\Apps\\Unfour\\unfour-mcp.exe"));
    expect(screen.getAllByRole("button", { name: copy.copied })).toHaveLength(2);
  });

  it.each(locales)("shows $locale clipboard failure and allows retry", async (copy) => {
    const writeText = vi.fn()
      .mockRejectedValueOnce(new Error("Clipboard permission denied"))
      .mockResolvedValueOnce(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    renderSettings(copy.locale);
    await waitFor(() => expect(screen.getByRole("button", { name: copy.command })).toBeEnabled());

    fireEvent.click(screen.getByRole("button", { name: copy.copy }));
    fireEvent.click(await screen.findByRole("button", { name: copy.failed }));

    expect(await screen.findByRole("button", { name: copy.copied })).toBeTruthy();
    expect(writeText).toHaveBeenCalledTimes(2);
    expect(mocks.configureMcpClient).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: copy.command })).toBeEnabled();
  });
});
