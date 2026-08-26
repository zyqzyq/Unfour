// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TerminalPage } from "./SshPage";

afterEach(cleanup);

vi.mock("./hooks/useSshConnections", () => ({
  useSshConnections: () => ({ data: [] }),
}));

vi.mock("./TerminalPage", () => ({
  SshConnectionsPage: ({
    active,
    onOpenTasks,
  }: {
    active: boolean;
    onOpenTasks: () => void;
  }) => (
    <div data-active={active} data-testid="connections-page">
      <button onClick={onOpenTasks} type="button">
        Open tasks
      </button>
    </div>
  ),
}));

vi.mock("./components/SshTasksPage", () => ({
  SshTasksPage: ({
    active,
    onOpenConnections,
  }: {
    active: boolean;
    onOpenConnections: () => void;
  }) => (
    <div data-active={active} data-testid="tasks-page">
      <button onClick={onOpenConnections} type="button">
        Open connections
      </button>
    </div>
  ),
}));

describe("SSH surface mode switching", () => {
  it("switches Connections and Tasks with one click", () => {
    render(<TerminalPage workspaceId="workspace-one" />);
    expect(screen.getByTestId("connections-page")).toHaveAttribute("data-active", "true");

    fireEvent.click(screen.getByRole("button", { name: "Open tasks" }));
    expect(screen.getByTestId("tasks-page")).toHaveAttribute("data-active", "true");
    expect(screen.getByTestId("connections-page")).toHaveAttribute("data-active", "false");

    fireEvent.click(screen.getByRole("button", { name: "Open connections" }));
    expect(screen.getByTestId("connections-page")).toHaveAttribute("data-active", "true");
    expect(screen.getByTestId("tasks-page")).toHaveAttribute("data-active", "false");
  });

  it("suspends both retained surfaces while the SSH module is inactive", () => {
    render(<TerminalPage active={false} workspaceId="workspace-one" />);

    expect(screen.getByTestId("connections-page")).toHaveAttribute("data-active", "false");
    expect(screen.getByTestId("tasks-page")).toHaveAttribute("data-active", "false");
  });
});
