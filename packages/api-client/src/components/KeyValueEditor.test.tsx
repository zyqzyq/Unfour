// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { KeyValueEditor } from "./KeyValueEditor";

afterEach(cleanup);

it("hides standard and configured API credentials without changing rows when revealed", () => {
  const change = vi.fn();
  render(<KeyValueEditor title="Headers" onChange={change} sensitiveKey="X-Custom-Credential"
    items={[
      { key: "Authorization", value: "Bearer {{token}}", enabled: true },
      { key: "X-Custom-Credential", value: "custom-secret", enabled: false },
      { key: "Accept", value: "application/json", enabled: true },
    ]} />);
  expect(screen.getByDisplayValue("Bearer {{token}}")).toHaveAttribute("type", "password");
  expect(screen.getByDisplayValue("custom-secret")).toHaveAttribute("type", "password");
  expect(screen.getByDisplayValue("application/json")).not.toHaveAttribute("type", "password");
  for (const button of screen.getAllByRole("button", { name: "Show secret value" })) fireEvent.click(button);
  expect(screen.getByDisplayValue("custom-secret")).toHaveAttribute("type", "text");
  expect(change).not.toHaveBeenCalled();
  fireEvent.change(screen.getByDisplayValue("custom-secret"), { target: { value: "replacement" } });
  expect(change.mock.lastCall?.[0][1]).toEqual({ key: "X-Custom-Credential", value: "replacement", enabled: false });
});
