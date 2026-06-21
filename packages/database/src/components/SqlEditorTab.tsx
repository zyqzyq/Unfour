import Editor, { type OnMount } from "@monaco-editor/react";
import { Eraser, Play, Square } from "lucide-react";
import { useEffect, useRef } from "react";
import type { DatabaseConnection, DatabaseSchema } from "@unfour/command-client";
import { Button, EmptyState, IconButton, Select, Toolbar, ToolbarGroup, useI18n } from "@unfour/ui";

type MonacoEditor = Parameters<OnMount>[0];

// Minimal structural types for the Monaco completion callback. The full
// `monaco-editor` types are not resolvable from this package, so we annotate
// only the members the provider actually reads.
type CompletionPosition = { lineNumber: number; column: number };
type CompletionModel = {
  getWordUntilPosition(position: CompletionPosition): { startColumn: number; endColumn: number };
};

export function SqlEditorTab({
  connections,
  executePending,
  onClearSql,
  onRun,
  onSelectConnection,
  onSqlChange,
  onStop,
  pendingConfirmation,
  schema,
  selectedConnectionId,
  sql,
}: {
  connections: DatabaseConnection[];
  executePending: boolean;
  onClearSql: () => void;
  onRun: (selectedSql?: string) => void;
  onSelectConnection: (connectionId: string) => void;
  onSqlChange: (sql: string) => void;
  onStop: () => void;
  pendingConfirmation: boolean;
  schema?: DatabaseSchema;
  selectedConnectionId: string | null;
  sql: string;
}) {
  const { t } = useI18n();
  const onRunRef = useRef(onRun);
  const editorRef = useRef<MonacoEditor | null>(null);
  const schemaRef = useRef<DatabaseSchema | undefined>(schema);
  const completionDisposable = useRef<{ dispose: () => void } | null>(null);
  const selectedConnection = connections.find((connection) => connection.id === selectedConnectionId);

  useEffect(() => {
    onRunRef.current = onRun;
  }, [onRun]);

  useEffect(() => {
    schemaRef.current = schema;
  }, [schema]);

  useEffect(() => () => completionDisposable.current?.dispose(), []);

  // Run the current selection when present, otherwise the full editor body.
  const runFromEditor = () => {
    const editor = editorRef.current;
    const selection = editor?.getSelection();
    const selected = selection ? editor?.getModel()?.getValueInRange(selection) : "";
    onRunRef.current(selected?.trim() ? selected : undefined);
  };

  const handleMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, runFromEditor);

    completionDisposable.current?.dispose();
    const provider = {
      provideCompletionItems(model: CompletionModel, position: CompletionPosition) {
        const word = model.getWordUntilPosition(position);
        const range = {
          startLineNumber: position.lineNumber,
          endLineNumber: position.lineNumber,
          startColumn: word.startColumn,
          endColumn: word.endColumn,
        };
        const tables = schemaRef.current?.tables ?? [];
        const suggestions: Array<{
          label: string;
          kind: number;
          insertText: string;
          detail?: string;
          range: typeof range;
        }> = [];
        const seenColumns = new Set<string>();
        for (const table of tables) {
          suggestions.push({
            label: table.name,
            kind: monaco.languages.CompletionItemKind.Struct,
            insertText: table.name,
            detail: table.schema ? `${table.schema} · ${table.kind}` : table.kind,
            range,
          });
          for (const column of table.columns) {
            if (seenColumns.has(column.name)) {
              continue;
            }
            seenColumns.add(column.name);
            suggestions.push({
              label: column.name,
              kind: monaco.languages.CompletionItemKind.Field,
              insertText: column.name,
              detail: column.dataType,
              range,
            });
          }
        }
        return { suggestions };
      },
    };
    completionDisposable.current = monaco.languages.registerCompletionItemProvider("sql", provider);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Toolbar className="h-8">
        <ToolbarGroup className="min-w-0 flex-1">
          <Select
            aria-label={t("database.editor.connectionAria")}
            className="max-w-[260px]"
            onChange={(event) => onSelectConnection(event.target.value)}
            options={connections.map((connection) => ({ label: connection.name, value: connection.id }))}
            value={selectedConnectionId ?? ""}
          >
            {!selectedConnectionId && <option value="">{t("database.connection.select")}</option>}
            {!connections.length && <option value="">{t("database.connection.none")}</option>}
          </Select>
          <span className="hidden min-w-0 truncate text-[12px] text-[var(--u-color-text-soft)] md:inline">
            {selectedConnection ? connectionContext(selectedConnection) : t("database.editor.noConnectionSelected")}
          </span>
        </ToolbarGroup>
        <ToolbarGroup>
          <IconButton disabled={!sql.trim() || executePending} label={t("database.actions.clearSql")} onClick={onClearSql}>
            <Eraser size={13} />
          </IconButton>
          <Button disabled={!selectedConnectionId || executePending} onClick={runFromEditor} size="sm" type="button">
            <Play size={13} />
            {pendingConfirmation ? t("database.actions.confirmRun") : t("database.actions.run")}
          </Button>
          <IconButton disabled={!executePending} label={t("database.actions.stopSql")} onClick={onStop}>
            <Square size={13} />
          </IconButton>
        </ToolbarGroup>
      </Toolbar>
      {connections.length === 0 ? (
        <EmptyState className="m-2 min-h-0 flex-1">{t("database.editor.empty")}</EmptyState>
      ) : (
        <Editor
          defaultLanguage="sql"
          onChange={(value) => onSqlChange(value ?? "")}
          onMount={handleMount}
          options={{
            fontSize: 13,
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            wordWrap: "on",
          }}
          value={sql}
        />
      )}
    </div>
  );
}

function connectionContext(connection: DatabaseConnection) {
  const database = connection.database ?? connection.sqlitePath ?? "default";
  return `${connection.driver} / ${database}`;
}
