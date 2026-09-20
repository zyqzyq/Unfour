// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { JsonField } from "./StepEditor";
afterEach(cleanup);
it("syncs externally changed values but retains invalid text on unrelated renders", () => {
  const onChange = vi.fn();
  const onValidity = vi.fn();
  const field = (value: unknown) => <I18nProvider initialLocale="en"><JsonField label="JSON" value={value} onChange={onChange} onValidity={onValidity} /></I18nProvider>;
  const view = render(field({ a: 1 }));
  fireEvent.change(screen.getByLabelText("JSON"), { target: { value: "{" } });
  view.rerender(field({ a: 1 }));
  expect(screen.getByLabelText("JSON")).toHaveValue("{");
  view.rerender(field({ b: 2 }));
  expect(screen.getByLabelText("JSON")).toHaveValue(JSON.stringify({ b: 2 }, null, 2));
  expect(screen.getByLabelText("JSON")).toHaveAttribute("aria-invalid", "false");
});
