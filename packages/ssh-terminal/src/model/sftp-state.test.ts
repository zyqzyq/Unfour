// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import {
  DEFAULT_SFTP_PANEL_WIDTH,
  MAX_SFTP_PANEL_WIDTH,
  MIN_SFTP_PANEL_WIDTH,
  clampSftpPanelWidth,
  maxSftpPanelWidth,
  useSftpStore,
} from "./sftp-state";

afterEach(() => {
  window.localStorage.clear();
  useSftpStore.setState({
    panelWidth: DEFAULT_SFTP_PANEL_WIDTH,
    tabs: {},
    transfers: {},
  });
});

describe("SFTP panel state", () => {
  it("clamps the resize width to a recoverable range and the available surface", () => {
    expect(clampSftpPanelWidth(10)).toBe(MIN_SFTP_PANEL_WIDTH);
    expect(clampSftpPanelWidth(2_000)).toBe(MAX_SFTP_PANEL_WIDTH);
    expect(clampSftpPanelWidth(800, 800)).toBe(480);
    expect(maxSftpPanelWidth(1_280)).toBe(MAX_SFTP_PANEL_WIDTH);
    expect(maxSftpPanelWidth(500)).toBe(MIN_SFTP_PANEL_WIDTH);
  });

  it("persists the last width and scopes open/path state by terminal session and connection", () => {
    const state = useSftpStore.getState();
    state.setPanelWidth(420);
    state.setPanelOpen("session-a", "connection-a", true);
    state.setPanelPath("session-a", "connection-a", "/srv/a");

    expect(window.localStorage.getItem("unfour.ssh.sftp-panel-width")).toBe("420");
    expect(useSftpStore.getState().tabs["session-a"]).toMatchObject({
      connectionId: "connection-a",
      open: true,
      path: "/srv/a",
    });
    expect(useSftpStore.getState().tabs["session-b"]).toBeUndefined();
  });
});
