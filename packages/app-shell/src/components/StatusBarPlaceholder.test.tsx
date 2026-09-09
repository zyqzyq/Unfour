// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { StatusBarPlaceholder } from "./StatusBarPlaceholder";

afterEach(cleanup);
const activeTab = { id: "api-main", title: "API", kind: "api" as const };

describe("shell status", () => {
  it("only reports known storage health, without fabricated connection or Git state", () => {
    const { rerender } = render(<StatusBarPlaceholder activeTab={activeTab} />);
    expect(screen.getByText("Checking storage")).toBeInTheDocument();
    rerender(<StatusBarPlaceholder activeTab={activeTab} healthReady />);
    expect(screen.getByText("Storage ready")).toBeInTheDocument();
    expect(screen.queryByText("Connected")).toBeNull();
    expect(screen.queryByText("main")).toBeNull();
    rerender(<StatusBarPlaceholder activeTab={activeTab} healthError />);
    expect(screen.getByText("Storage unavailable")).toBeInTheDocument();
    rerender(<StatusBarPlaceholder activeTab={activeTab} healthReady={false} />);
    expect(screen.getByText("Storage unavailable")).toBeInTheDocument();
  });
});
