// @vitest-environment jsdom
import { useEffect, useRef } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { executeDatabaseScript, listDatabaseConnections } from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { DatabasePage } from "./DatabasePage";
import { connection } from "./hooks/database-controller.test-fixtures";
import { useDatabaseConnectionStore } from "./model/database-connection-state";
import { resetDatabaseTabStore, useDatabaseTabStore } from "./model/database-tab-state";

vi.mock("@monaco-editor/react", () => ({
  default: function MockEditor({
    onMount,
    value,
  }: {
    onMount?: (editor: unknown, monaco: unknown) => void;
    value?: string;
  }) {
    const onMountRef = useRef(onMount);
    onMountRef.current = onMount;
    useEffect(() => {
      onMountRef.current?.({
        addCommand: vi.fn(),
        getModel: () => ({ getOffsetAt: () => 0, getValueInRange: () => "" }),
        getPosition: () => ({ column: 1, lineNumber: 1 }),
        getSelection: () => ({ endColumn: 1, endLineNumber: 1, startColumn: 1, startLineNumber: 1 }),
        layout: vi.fn(),
        onDidChangeCursorSelection: () => ({ dispose: vi.fn() }),
      }, {
        KeyCode: { Enter: 3 },
        KeyMod: { CtrlCmd: 2048, Shift: 1024 },
        editor: { setTheme: vi.fn() },
        languages: {
          CompletionItemKind: { Field: 4, Struct: 6 },
          registerCompletionItemProvider: () => ({ dispose: vi.fn() }),
        },
      });
    }, []);
    return <textarea aria-label="sql editor" readOnly value={value} />;
  },
}));

vi.mock("./components/sql-editor-theme", () => ({ configureSqlEditorThemes: vi.fn() }));

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  executeDatabaseScript: vi.fn(),
  getDatabaseSchema: vi.fn(async () => ({ connectionId: "pg", tables: [] })),
  listDatabaseCatalogs: vi.fn(async () => []),
  listDatabaseConnections: vi.fn(),
  listDatabaseQueryHistory: vi.fn(async () => []),
  listSavedSql: vi.fn(async () => []),
  recordDatabaseQueryHistory: vi.fn(async () => undefined),
  stopDatabaseScript: vi.fn(),
}));

const execute = vi.mocked(executeDatabaseScript);

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    unobserve() {}
    disconnect() {}
  });
  resetDatabaseTabStore();
  useWorkspaceStore.setState({ selectedDatabaseConnectionId: "pg" });
  useDatabaseConnectionStore.setState({ byWorkspace: { ws: { pg: { status: "connected" } } } });
  useDatabaseTabStore.getState().openQueryTab("ws", {
    connectionId: "pg",
    sql: "INSERT INTO effects(n) VALUES (1);",
  });
  vi.mocked(listDatabaseConnections).mockResolvedValue([connection("pg", "postgres")]);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("shows the SQL editor activity warning without rerunning the script", async () => {
  execute.mockResolvedValue({
    stopped: false,
    warnings: ["database.batch.activityFailed"],
    statements: [{
      index: 1,
      start: 0,
      end: 34,
      sql: "INSERT INTO effects(n) VALUES (1);",
      status: "success",
      error: null,
      result: {
        columns: [{ name: "n", dataType: "int" }],
        rows: [["kept-row"]],
        affectedRows: 1,
        durationMs: 3,
        truncated: false,
        safety: { classification: "write", confirmed: true, requiresConfirmation: false, message: null },
      },
    }],
  });
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <I18nProvider initialLocale="en" storageKey="sql-activity-test">
        <DatabasePage workspaceId="ws" workspaceName="Workspace" />
      </I18nProvider>
    </QueryClientProvider>,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Run All" }));
  expect(await screen.findByText("kept-row")).toBeVisible();
  expect(screen.getByText(/local activity logging failed/)).toBeVisible();
  const query = useDatabaseTabStore.getState().byWorkspace.ws.tabs.find(
    (tab) => tab.kind === "query" && tab.result?.rows?.[0]?.[0] === "kept-row",
  );
  expect(query && query.kind === "query" ? query.executionNotice : "").toContain(
    "Do not rerun completed mutations just to repair the log.",
  );
  expect(screen.queryByText(/audit sink/i)).not.toBeInTheDocument();
  expect(execute).toHaveBeenCalledTimes(1);
  expect(execute.mock.calls[0][0]).toEqual(expect.objectContaining({
    confirmMutation: false,
    connectionId: "pg",
    sql: "INSERT INTO effects(n) VALUES (1);",
  }));
  await waitFor(() => expect(execute).toHaveBeenCalledTimes(1));
});
