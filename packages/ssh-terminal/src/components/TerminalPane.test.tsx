// @vitest-environment jsdom
import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import {
  resetTerminalMocks,
  resizeMock,
  sendInputMock,
  session,
  terminalState,
} from "./terminal-pane-test-harness";
import { TerminalPane } from "./TerminalPane";

describe("TerminalPane", () => {
  beforeEach(resetTerminalMocks);

  it("syncs the fitted terminal size to the SSH session even without an xterm resize event", async () => {
    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    await waitFor(() =>
      expect(resizeMock).toHaveBeenCalledWith({
        workspaceId: "ws-1",
        sessionId: "session-1",
        cols: 96,
        rows: 28,
      }),
    );
  });

  it("opens xterm in an unpadded fit host so padding cannot overflow the pane", () => {
    render(
      <TerminalPane
        active
        events={[]}
        inputDisabled={false}
        readOnly={false}
        session={session}
      />,
    );

    expect(terminalState.openElement).not.toBeNull();
    expect(terminalState.openElement).not.toHaveClass("p-2");
    expect(terminalState.openElement).toHaveClass("h-full", "w-full", "overflow-hidden");
    expect(terminalState.openElement?.parentElement).toHaveClass("p-2");
  });

  it("serializes terminal input chunks for interactive programs", async () => {
    let resolveFirst: (() => void) | null = null;
    sendInputMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = () =>
            resolve({
              sessionId: "session-1",
              kind: "output",
              data: "first accepted\r\n",
              createdAt: "2026-06-23T00:00:01.000Z",
            });
        }),
    );
    sendInputMock.mockResolvedValue({
      sessionId: "session-1",
      kind: "output",
      data: "next accepted\r\n",
      createdAt: "2026-06-23T00:00:02.000Z",
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

    terminalState.dataHandlers[0]?.("i");
    terminalState.dataHandlers[0]?.("hello");

    await waitFor(() => expect(sendInputMock).toHaveBeenCalledTimes(1));
    expect(sendInputMock).toHaveBeenLastCalledWith({
      workspaceId: "ws-1",
      sessionId: "session-1",
      data: "i",
    });

    resolveFirst?.();

    await waitFor(() => expect(sendInputMock).toHaveBeenCalledTimes(2));
    expect(sendInputMock).toHaveBeenLastCalledWith({
      workspaceId: "ws-1",
      sessionId: "session-1",
      data: "hello",
    });
  });
});
