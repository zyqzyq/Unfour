// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TerminalWorkspace } from "./TerminalWorkspace";

vi.mock("./SftpWorkspace", () => ({
  SftpWorkspace: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock("./TerminalSearchBar", () => ({ TerminalSearchBar: () => null }));

vi.mock("./TerminalSplitView", () => ({ TerminalSplitView: () => null }));

vi.mock("../hooks/useTerminalSplit", () => ({
  useTerminalSplit: () => ({ setMode: vi.fn() }),
}));

afterEach(cleanup);

function workspaceProps(
  overrides: Partial<Parameters<typeof TerminalWorkspace>[0]> = {},
): Parameters<typeof TerminalWorkspace>[0] {
  return {
    activeSession: null,
    activeSessionId: null,
    emptyMessage: "No saved connections",
    events: [],
    onClear: vi.fn(),
    onCloseAll: vi.fn(),
    onCloseLeft: vi.fn(),
    onCloseOthers: vi.fn(),
    onCloseRight: vi.fn(),
    onCloseSession: vi.fn(),
    onDuplicate: vi.fn(),
    onNewConnection: vi.fn(),
    onNewSession: vi.fn(),
    onOpenPreferences: vi.fn(),
    onReconnect: vi.fn(),
    onRetry: vi.fn(),
    onSelectSession: vi.fn(),
    selectedConnection: null,
    sessions: [],
    splitMode: "single",
    ...overrides,
  };
}

describe("TerminalWorkspace progressive loading", () => {
  it.each([
    ["en", "Close Connection", "Close Others", "Close to the Right"],
    ["zh-CN", "关闭连接", "关闭其他", "关闭右边"],
  ] as const)("keeps close actions with the session owner and removes unsupported hints (%s)", (locale, close, others, right) => {
    const sessions = ["one", "two"].map((id) => ({
      title: id,
      session: { sessionId: id, connectionId: id, workspaceId: "ws-1", status: "connected" as const, reconnectAttempt: 0, authKind: "password", host: id, username: "ops", cols: 80, rows: 24, createdAt: "2026-01-01T00:00:00Z", updatedAt: "2026-01-01T00:00:00Z" },
    }));
    const props = workspaceProps({ sessions });
    render(<I18nProvider initialLocale={locale}><TerminalWorkspace {...props} /></I18nProvider>);
    for (const [label, callback] of [[close, props.onCloseSession], [others, props.onCloseOthers], [right, props.onCloseRight]] as const) {
      fireEvent.contextMenu(screen.getByRole("tab", { name: /ops@one/ }));
      expect(screen.queryByText("Ctrl+W")).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole("menuitem", { name: label }));
      expect(callback).toHaveBeenCalledExactlyOnceWith("one");
    }
  });

  it("keeps the SSH workspace mounted while showing local loading feedback", () => {
    const { rerender } = render(
      <I18nProvider initialLocale="en">
        <TerminalWorkspace {...workspaceProps({ loading: true })} />
      </I18nProvider>,
    );

    expect(screen.getByText("Loading terminal workspace...")).toBeTruthy();

    rerender(
      <I18nProvider initialLocale="en">
        <TerminalWorkspace {...workspaceProps({ loading: false })} />
      </I18nProvider>,
    );
    expect(screen.getByRole("button", { name: "New Connection" })).toBeTruthy();
  });

  it("shows the full SSH action error and copies it", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    const longError =
      "ssh: handshake failed: remote protocol version is not supported\n" +
      Array.from({ length: 8 }, () => "detail: cipher list rejected by server").join(" ");

    render(
      <I18nProvider initialLocale="en">
        <TerminalWorkspace {...workspaceProps({ actionError: new Error(longError) })} />
      </I18nProvider>,
    );

    const alert = screen.getByRole("alert");
    const message = alert.querySelector("p");
    expect(alert).toHaveTextContent("ssh: handshake failed");
    expect(alert).toHaveTextContent("cipher list rejected by server");
    expect(message).toHaveClass("whitespace-pre-wrap");
    expect(message).not.toHaveClass("truncate");
    expect(message).toHaveAttribute("title", longError);

    fireEvent.click(screen.getByRole("button", { name: "Copy error" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith(longError));
    vi.unstubAllGlobals();
  });
});
