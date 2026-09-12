// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { DatabaseConnection } from "@unfour/command-client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SqlEditorTab } from "./SqlEditorTab";

const editorState = {
  commands: [] as Array<{ handler: () => void; keybinding: number }>,
  cursorOffset: 0,
  notifySelection: () => undefined as void,
  selection: "",
};

vi.mock("@monaco-editor/react", async () => {
  const { useEffect, useRef } = await import("react");
  return {
    default: function MockEditor({
      onChange,
      onMount,
      value,
    }: {
      onChange?: (value: string | undefined) => void;
      onMount?: (editor: unknown, monaco: unknown) => void;
      value?: string;
    }) {
      const onMountRef = useRef(onMount);
      onMountRef.current = onMount;
      useEffect(() => {
        const editor = {
          addCommand(keybinding: number, handler: () => void) {
            editorState.commands.push({ handler, keybinding });
          },
          getModel: () => ({
            getOffsetAt: () => editorState.cursorOffset,
            getValueInRange: () => editorState.selection,
          }),
          getPosition: () => ({ column: 1, lineNumber: 1 }),
          getSelection: () => ({ endColumn: 1, endLineNumber: 1, startLineNumber: 1, startColumn: 1 }),
          layout: vi.fn(),
          onDidChangeCursorSelection(listener: () => void) {
            editorState.notifySelection = listener;
            listener();
            return { dispose: vi.fn() };
          },
        };
        const monaco = {
          KeyCode: { Enter: 4 },
          KeyMod: { CtrlCmd: 1, Shift: 2 },
          editor: { setTheme: vi.fn() },
          languages: {
            CompletionItemKind: { Field: 2, Struct: 1 },
            registerCompletionItemProvider: () => ({ dispose: vi.fn() }),
          },
        };
        onMountRef.current?.(editor, monaco);
      }, []);
      return (
        <textarea
          aria-label="sql editor"
          onChange={(event) => onChange?.(event.target.value)}
          value={value}
        />
      );
    },
  };
});

vi.mock("./sql-editor-theme", () => ({
  configureSqlEditorThemes: vi.fn(),
}));

vi.mock("../hooks/useSavedSql", () => ({
  useSavedSql: () => ({
    error: null,
    isLoading: false,
    remove: vi.fn(),
    removePending: false,
    save: vi.fn(),
    saved: [],
    savePending: false,
  }),
}));

afterEach(cleanup);
beforeEach(() => {
  editorState.commands = [];
  editorState.cursorOffset = 12;
  editorState.notifySelection = () => undefined;
  editorState.selection = "";
});

const connection: DatabaseConnection = {
  createdAt: "2026-01-01T00:00:00.000Z",
  credentialRef: null,
  database: null,
  deletedAt: null,
  driver: "sqlite",
  host: null,
  id: "conn-1",
  name: "Local SQLite",
  port: null,
  readOnly: false,
  remoteId: null,
  revision: 1,
  sqlitePath: "D:\\data\\app.sqlite",
  sslMode: null,
  syncStatus: "local",
  updatedAt: "2026-01-01T00:00:00.000Z",
  username: null,
  workspaceId: "ws-1",
};

const MULTI_SQL = "SELECT 1;\nDELETE FROM t;\nSELECT 2;";
const SELECTED_SQL = "DELETE FROM t;";

function renderEditor(props: Partial<Parameters<typeof SqlEditorTab>[0]> = {}) {
  const onRun = vi.fn();
  const onStop = vi.fn();
  const view = render(
    <SqlEditorTab
      active
      catalogOptions={[]}
      connections={[connection]}
      executePending={false}
      onChangeQueryContext={vi.fn()}
      onClearSql={vi.fn()}
      onRun={onRun}
      onSelectConnection={vi.fn()}
      onShowHistory={vi.fn()}
      onSqlChange={vi.fn()}
      onStop={onStop}
      pendingConfirmation={false}
      queryCatalog={null}
      querySchema={null}
      schemaOptions={[]}
      selectedConnectionId="conn-1"
      sql={MULTI_SQL}
      workspaceId="ws-1"
      {...props}
    />,
  );
  return { ...view, onRun, onStop };
}

function setSelection(sql: string) {
  editorState.selection = sql;
  act(() => editorState.notifySelection());
}

describe("SQL editor run actions", () => {
  it("runs the full editor SQL on Run All without cursor or selection", () => {
    const { onRun } = renderEditor();
    fireEvent.click(screen.getByRole("button", { name: "Run All" }));
    expect(onRun).toHaveBeenCalledTimes(1);
    expect(onRun).toHaveBeenCalledWith({ mode: "all" });
  });

  it("runs only the selection on Run Selected", () => {
    const { onRun } = renderEditor();
    setSelection(SELECTED_SQL);
    fireEvent.click(screen.getByRole("button", { name: "Run Selected" }));
    expect(onRun).toHaveBeenCalledTimes(1);
    expect(onRun).toHaveBeenCalledWith({ sql: SELECTED_SQL });
  });

  it("disables Run Selected without a selection and does not send a request", () => {
    const { onRun } = renderEditor();
    const runSelected = screen.getByRole("button", { name: "Run Selected" });
    expect(runSelected).toBeDisabled();
    fireEvent.click(runSelected);
    expect(onRun).not.toHaveBeenCalled();
  });

  it("still runs the full editor SQL on Run All when a selection exists", () => {
    const { onRun } = renderEditor();
    setSelection(SELECTED_SQL);
    fireEvent.click(screen.getByRole("button", { name: "Run All" }));
    expect(onRun).toHaveBeenCalledWith({ mode: "all" });
  });

  it("disables both run buttons when SQL is empty", () => {
    renderEditor({ sql: "   " });
    expect(screen.getByRole("button", { name: "Run All" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Run Selected" })).toBeDisabled();
  });

  it("replaces both run buttons with Stop while execution is in progress", () => {
    const { onRun, onStop } = renderEditor({ executePending: true });
    expect(screen.queryByRole("button", { name: "Run All" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Run Selected" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Stop SQL execution" }));
    expect(onStop).toHaveBeenCalledTimes(1);
    expect(onRun).not.toHaveBeenCalled();
  });

  it("resumes the stored batch from Confirm without rereading editor state", () => {
    const { onRun } = renderEditor({ pendingConfirmation: true });
    setSelection(SELECTED_SQL);
    fireEvent.click(screen.getByRole("button", { name: "Confirm run" }));
    expect(onRun).toHaveBeenCalledWith({ resume: true });
    expect(screen.queryByRole("button", { name: "Run All" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Run Current" })).not.toBeInTheDocument();
  });

  it("keeps Explain on current-statement / selection semantics", () => {
    const { onRun } = renderEditor();
    fireEvent.click(screen.getByRole("button", { name: "Explain" }));
    expect(onRun).toHaveBeenCalledWith({
      cursorOffset: 12,
      explain: true,
      mode: "current",
      sql: undefined,
    });
    onRun.mockClear();
    setSelection(SELECTED_SQL);
    fireEvent.click(screen.getByRole("button", { name: "Explain" }));
    expect(onRun).toHaveBeenCalledWith({
      cursorOffset: 12,
      explain: true,
      mode: "current",
      sql: SELECTED_SQL,
    });
  });

  it("does not keep a Run Current dropdown", () => {
    renderEditor();
    expect(screen.queryByRole("button", { name: "Run Current" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Run options")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run All" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Run Selected" })).toBeInTheDocument();
  });
});
