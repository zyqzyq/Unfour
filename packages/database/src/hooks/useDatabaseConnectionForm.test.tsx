// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import type { DatabaseConnection } from "@unfour/command-client";
import { useDatabaseConnectionForm } from "./useDatabaseConnectionForm";

afterEach(cleanup);

const connection: DatabaseConnection = {
  id: "db-a",
  workspaceId: "ws-a",
  name: "DB",
  driver: "postgres",
  host: "localhost",
  port: 5432,
  database: "app",
  username: "dev",
  sslMode: null,
  sqlitePath: null,
  credentialRef: "credential-a",
  readOnly: false,
  createdAt: "",
  updatedAt: "",
  deletedAt: null,
  revision: 1,
  syncStatus: "local",
  remoteId: null,
};

it("preserves unsaved edits through cache renders, but clears credentials on connection/workspace changes", () => {
  const { result, rerender } = renderHook(
    ({
      workspaceId,
      selected,
    }: {
      workspaceId: string;
      selected: DatabaseConnection | null;
    }) => useDatabaseConnectionForm(workspaceId, selected?.id ?? null, selected),
    { initialProps: { workspaceId: "ws-a", selected: null as DatabaseConnection | null } },
  );
  rerender({ workspaceId: "ws-a", selected: connection });
  act(() => {
    result.current.setForm((form) => ({ ...form, name: "Unsaved name" }));
    result.current.setPassword("typed secret");
    result.current.setEditorOpen(true);
  });
  rerender({
    workspaceId: "ws-a",
    selected: { ...connection, credentialRef: "credential-from-cache", name: "Cache name" },
  });
  expect(result.current.form.name).toBe("Unsaved name");
  expect(result.current.form.credentialRef).toBe("credential-a");
  expect(result.current.password).toBe("typed secret");
  expect(result.current.editorOpen).toBe(true);
  rerender({
    workspaceId: "ws-a",
    selected: { ...connection, id: "db-b", credentialRef: "credential-b" },
  });
  expect(result.current.form.id).toBe("db-b");
  expect(result.current.password).toBe("");
  act(() => result.current.setPassword("another secret"));
  rerender({ workspaceId: "ws-b", selected: null });
  expect(result.current.form).toEqual({
    workspaceId: "ws-b",
    name: "",
    driver: "sqlite",
    sqlitePath: "",
  });
  expect(result.current.password).toBe("");
  expect(result.current.editorOpen).toBe(false);
  expect(result.current.testResult).toBeNull();
});

it("rehydrates from an explicit Edit even when the selected connection id is unchanged", () => {
  const { result, rerender } = renderHook(
    ({ selected }: { selected: DatabaseConnection | null }) =>
      useDatabaseConnectionForm("ws-a", selected?.id ?? null, selected),
    { initialProps: { selected: null as DatabaseConnection | null } },
  );
  rerender({ selected: connection });

  act(() => {
    result.current.setForm((form) => ({ ...form, name: "Draft name", host: "draft.local" }));
    result.current.setPassword("draft-secret");
  });

  const saved: DatabaseConnection = {
    ...connection,
    name: "Saved name",
    credentialRef: "credential-saved",
    revision: 2,
  };
  act(() => {
    result.current.hydrateFormFromConnection(saved);
    result.current.setEditorOpen(true);
  });
  expect(result.current.form.name).toBe("Saved name");
  expect(result.current.form.credentialRef).toBe("credential-saved");
  expect(result.current.password).toBe("");
  expect(result.current.editorOpen).toBe(true);

  act(() => {
    result.current.setForm((form) => ({ ...form, database: "draft-db" }));
  });
  rerender({ selected: { ...saved, database: "cache-db", host: "cache.local" } });
  expect(result.current.form.database).toBe("draft-db");
  expect(result.current.form.host).toBe("localhost");
  expect(result.current.form.credentialRef).toBe("credential-saved");
});
