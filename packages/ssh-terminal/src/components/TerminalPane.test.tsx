// @vitest-environment jsdom
import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import {
  resetTerminalMocks,
  resizeMock,
  sendInputMock,
  session,
  terminalState,
  listHistoryMock,
  temporaryInputMock,
  temporaryResizeMock,
} from "./terminal-pane-test-harness";
import { TerminalPane } from "./TerminalPane";

describe("TerminalPane", () => {
  beforeEach(resetTerminalMocks);

  it.each(["task", "unmount"])("drops queued temporary keystrokes after %s invalidates manual input", async (change) => {
    let finish!: () => void;
    temporaryInputMock.mockImplementationOnce(() => new Promise((resolve) => {
      finish = () => resolve({ sessionId: "session-1", kind: "input", data: "", createdAt: "" });
    }));
    const mounted = render(<TerminalPane active temporary events={[]} session={session} />);
    terminalState.dataHandlers[0]?.("first");
    terminalState.dataHandlers[0]?.("queued-before-task");
    await waitFor(() => expect(temporaryInputMock).toHaveBeenCalledTimes(1));
    if (change === "task") {
      mounted.rerender(<TerminalPane active temporary inputDisabled events={[]} session={session} />);
      // The old chunk remains invalid even after the task has released the terminal.
      mounted.rerender(<TerminalPane active temporary inputDisabled={false} events={[]} session={session} />);
    } else mounted.unmount();
    await act(async () => { finish(); });
    expect(temporaryInputMock).toHaveBeenCalledTimes(1);
    if (change === "task") {
      terminalState.dataHandlers[0]?.("fresh-after-task");
      await waitFor(() => expect(temporaryInputMock).toHaveBeenCalledTimes(2));
      expect(temporaryInputMock).toHaveBeenLastCalledWith({ workspaceId: "ws-1", sessionId: "session-1", data: "fresh-after-task" });
    }
  });

  it("routes temporary I/O separately without saved history or endpoint display", async () => {
    render(<TerminalPane active temporary events={[]} session={session} />);
    await waitFor(() => expect(temporaryResizeMock).toHaveBeenCalled());
    terminalState.dataHandlers.forEach((handler) => handler("synthetic-input"));
    await waitFor(() => expect(temporaryInputMock).toHaveBeenCalledWith({ workspaceId: "ws-1", sessionId: "session-1", data: "synthetic-input" }));
    expect(listHistoryMock).not.toHaveBeenCalled();
    expect(sendInputMock).not.toHaveBeenCalled();
    expect(resizeMock).not.toHaveBeenCalled();
    expect(terminalState.writes.join("")).not.toContain("example.test");
  });

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

  it("sends the already fitted size when the temporary PTY becomes writable", async () => {
    const { rerender } = render(
      <TerminalPane active temporary inputDisabled events={[]} session={{ ...session, status: "disconnected" }} />,
    );
    await waitFor(() => expect(terminalState.refreshCalls).toBeGreaterThan(0));
    expect([terminalState.cols, terminalState.rows]).toEqual([96, 28]);
    expect(temporaryResizeMock).not.toHaveBeenCalled();

    rerender(<TerminalPane active temporary inputDisabled={false} events={[]} session={session} />);
    await waitFor(() => expect(temporaryResizeMock).toHaveBeenCalledExactlyOnceWith({
      workspaceId: session.workspaceId, sessionId: session.sessionId, cols: 96, rows: 28,
    }));
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
    expect(terminalState.openElement).not.toHaveClass("p-4");
    expect(terminalState.openElement).toHaveClass("h-full", "w-full", "overflow-hidden");
    expect(terminalState.openElement?.parentElement).toHaveClass("p-4");
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
