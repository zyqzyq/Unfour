// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
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
});
