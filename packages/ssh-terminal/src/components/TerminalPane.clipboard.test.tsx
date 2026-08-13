// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import {
  clipboardReadMock,
  clipboardWriteMock,
  resetTerminalMocks,
  sendInputMock,
  session,
  terminalState,
} from "./terminal-pane-test-harness";
import { TerminalPane } from "./TerminalPane";

describe("TerminalPane clipboard interactions", () => {
  beforeEach(resetTerminalMocks);

  it("copies the terminal selection from the custom context menu", async () => {
    terminalState.selection = "selected terminal output";
    clipboardWriteMock.mockResolvedValue(undefined);

    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    fireEvent.click(
      screen.getByRole("menuitem", { name: /ssh\.actions\.copySelection/ }),
    );

    await waitFor(() =>
      expect(clipboardWriteMock).toHaveBeenCalledWith("selected terminal output"),
    );
    expect(clipboardWriteMock).toHaveBeenCalledTimes(1);
    expect(sendInputMock).not.toHaveBeenCalled();
  });

  it("replaces the native menu and pastes the terminal selection exactly once", async () => {
    terminalState.selection = "selected terminal output";
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:03.000Z",
    });

    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    fireEvent.click(
      screen.getByRole("menuitem", { name: "ssh.actions.pasteSelection" }),
    );

    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "selected terminal output",
      }),
    );
    expect(terminalState.pasteCalls).toEqual(["selected terminal output"]);
    expect(sendInputMock).toHaveBeenCalledTimes(1);
  });

  it("disables selection paste without a selection and supports clipboard paste and select all", async () => {
    clipboardReadMock.mockResolvedValue("pasted command\r");
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:03.000Z",
    });

    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    expect(
      screen.getByRole("menuitem", { name: "ssh.actions.pasteSelection" }),
    ).toHaveAttribute("data-disabled");

    fireEvent.click(
      screen.getByRole("menuitem", { name: /ssh\.actions\.pasteClipboard/ }),
    );
    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "pasted command\r",
      }),
    );
    expect(terminalState.pasteCalls).toEqual(["pasted command\r"]);
    expect(sendInputMock).toHaveBeenCalledTimes(1);

    fireEvent.contextMenu(terminalState.inputElement as HTMLTextAreaElement, {
      clientX: 24,
      clientY: 24,
    });
    fireEvent.click(screen.getByRole("menuitem", { name: "ssh.actions.selectAll" }));
    expect(terminalState.selectAllCalls).toBe(1);
  });

  it("leaves Ctrl+V to xterm's single native paste path", async () => {
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "",
      createdAt: "2026-06-23T00:00:04.000Z",
    });

    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    expect(terminalState.customKeyHandlerRegistrations).toBe(1);
    expect(
      terminalState.customKeyHandlers[0]?.(
        new KeyboardEvent("keydown", { key: "v", ctrlKey: true }),
      ),
    ).toBe(true);
    terminalState.dataHandlers[0]?.("native paste once");

    await waitFor(() =>
      expect(sendInputMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        data: "native paste once",
      }),
    );
    expect(sendInputMock).toHaveBeenCalledTimes(1);
  });
});
