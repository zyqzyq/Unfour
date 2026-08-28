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

function renderSettings() {
  return render(
    <I18nProvider initialLocale="en" storageKey="test.settings-mcp.locale">
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
  });
});
