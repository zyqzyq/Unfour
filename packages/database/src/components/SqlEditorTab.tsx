import Editor, { type OnMount } from "@monaco-editor/react";
import { AlignLeft, Eraser, History, Info, Play, Save, Square, Star, Trash2 } from "lucide-react";
import { type FormEvent, useEffect, useRef, useState } from "react";
import type { DatabaseConnection, DatabaseSchema, SavedSql } from "@unfour/command-client";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  EmptyState,
  ErrorState,
  IconButton,
  Input,
  Select,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import { useSavedSql } from "../hooks/useSavedSql";
import { formatSql } from "../result-utils";
import { formatDatabaseError } from "../result-utils";

type MonacoEditor = Parameters<OnMount>[0];

// Minimal structural types for the Monaco completion callback. The full
// `monaco-editor` types are not resolvable from this package, so we annotate
// only the members the provider actually reads.
type CompletionPosition = { lineNumber: number; column: number };
type CompletionModel = {
  getWordUntilPosition(position: CompletionPosition): { startColumn: number; endColumn: number };
};

export function SqlEditorTab({
  catalogOptions,
  connections,
  executePending,
  onChangeQueryContext,
  onClearSql,
  onRun,
  onSelectConnection,
  onShowHistory,
  onSqlChange,
  onStop,
  pendingConfirmation,
  queryCatalog,
  querySchema,
  schema,
  schemaOptions,
  selectedConnectionId,
  sql,
  workspaceId,
}: {
  catalogOptions: string[];
  connections: DatabaseConnection[];
  executePending: boolean;
  onChangeQueryContext: (patch: { catalog?: string | null; schema?: string | null }) => void;
  onClearSql: () => void;
  onRun: (selectedSql?: string) => void;
  onSelectConnection: (connectionId: string) => void;
  onShowHistory: () => void;
  onSqlChange: (sql: string) => void;
  onStop: () => void;
  pendingConfirmation: boolean;
  queryCatalog: string | null;
  querySchema: string | null;
  schema?: DatabaseSchema;
  schemaOptions: string[];
  selectedConnectionId: string | null;
  sql: string;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const onRunRef = useRef(onRun);
  const editorRef = useRef<MonacoEditor | null>(null);
  const schemaRef = useRef<DatabaseSchema | undefined>(schema);
  const completionDisposable = useRef<{ dispose: () => void } | null>(null);
  const selectedConnection = connections.find((connection) => connection.id === selectedConnectionId);
  const savedSql = useSavedSql(workspaceId);
  const [saveDialogOpen, setSaveDialogOpen] = useState(false);
  const [savedDialogOpen, setSavedDialogOpen] = useState(false);
  const [snippetName, setSnippetName] = useState("");

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

  // EXPLAIN the current selection (or the whole body) without mutating the
  // editor text. The wrapped statement is read-only, so it bypasses confirmation.
  const explainFromEditor = () => {
    const editor = editorRef.current;
    const selection = editor?.getSelection();
    const selected = selection ? editor?.getModel()?.getValueInRange(selection) : "";
    const base = (selected?.trim() ? selected : sql).trim();
    if (!base) {
      return;
    }
    onRunRef.current(`EXPLAIN ${base}`);
  };

  const formatEditor = () => {
    if (!sql.trim()) {
      return;
    }
    onSqlChange(formatSql(sql));
  };

  const openSaveDialog = () => {
    setSnippetName(defaultSavedSqlName(sql, t("database.saved.defaultName")));
    setSaveDialogOpen(true);
  };

  const submitSavedSql = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const name = snippetName.trim();
    if (!name || !sql.trim()) {
      return;
    }
    await savedSql.save({
      connectionId: selectedConnectionId,
      name,
      sql,
      workspaceId,
    });
    setSaveDialogOpen(false);
  };

  const loadSavedSql = (item: SavedSql) => {
    onSqlChange(item.sql);
    if (item.connectionId && connections.some((connection) => connection.id === item.connectionId)) {
      onSelectConnection(item.connectionId);
    }
    setSavedDialogOpen(false);
  };

  const deleteSavedSql = (item: SavedSql) => {
    void savedSql.remove(item.id);
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
            detail: [table.catalog, table.schema, table.kind].filter(Boolean).join(" · "),
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
      <Toolbar className="h-9">
        <ToolbarGroup>
          <Button disabled={!selectedConnectionId || executePending} onClick={runFromEditor} size="sm" type="button">
            <Play size={13} />
            {pendingConfirmation ? t("database.actions.confirmRun") : t("database.actions.run")}
          </Button>
          <Button
            disabled={!selectedConnectionId || !sql.trim() || executePending}
            onClick={explainFromEditor}
            size="sm"
            type="button"
            variant="ghost"
          >
            <Info size={13} />
            {t("database.actions.explain")}
          </Button>
          <Button disabled={!sql.trim()} onClick={formatEditor} size="sm" type="button" variant="ghost">
            <AlignLeft size={13} />
            {t("database.actions.format")}
          </Button>
          <Button onClick={onShowHistory} size="sm" type="button" variant="ghost">
            <History size={13} />
            {t("database.actions.history")}
          </Button>
          <Button
            disabled={!sql.trim() || savedSql.savePending}
            onClick={openSaveDialog}
            size="sm"
            type="button"
            variant="ghost"
          >
            <Save size={13} />
            {savedSql.savePending ? t("database.saved.saving") : t("database.actions.saveSql")}
          </Button>
          <Button onClick={() => setSavedDialogOpen(true)} size="sm" type="button" variant="ghost">
            <Star size={13} />
            {t("database.actions.savedSql")}
          </Button>
          <IconButton disabled={!sql.trim() || executePending} label={t("database.actions.clearSql")} onClick={onClearSql}>
            <Eraser size={13} />
          </IconButton>
          <IconButton disabled={!executePending} label={t("database.actions.stopSql")} onClick={onStop}>
            <Square size={13} />
          </IconButton>
        </ToolbarGroup>
        <ToolbarGroup className="min-w-0">
          <Select
            aria-label={t("database.editor.connectionAria")}
            className="max-w-[220px]"
            onChange={(event) => onSelectConnection(event.target.value)}
            options={connections.map((connection) => ({ label: connection.name, value: connection.id }))}
            value={selectedConnectionId ?? ""}
          >
            {!selectedConnectionId && <option value="">{t("database.connection.select")}</option>}
            {!connections.length && <option value="">{t("database.connection.none")}</option>}
          </Select>
          {catalogOptions.length > 0 && (
            <Select
              aria-label={t("database.editor.catalogAria")}
              className="max-w-[160px]"
              onChange={(event) => onChangeQueryContext({ catalog: event.target.value || null })}
              options={catalogOptions.map((catalog) => ({ label: catalog, value: catalog }))}
              value={queryCatalog ?? ""}
            />
          )}
          {schemaOptions.length > 0 && (
            <Select
              aria-label={t("database.editor.schemaAria")}
              className="max-w-[160px]"
              onChange={(event) => onChangeQueryContext({ schema: event.target.value || null })}
              options={schemaOptions.map((schema) => ({ label: schema, value: schema }))}
              value={querySchema ?? ""}
            />
          )}
          <span className="hidden min-w-0 truncate text-[12px] text-[var(--u-color-text-soft)] lg:inline">
            {selectedConnection ? connectionContext(selectedConnection) : t("database.editor.noConnectionSelected")}
          </span>
        </ToolbarGroup>
      </Toolbar>
      {connections.length === 0 ? (
        <EmptyState className="m-2 min-h-0 flex-1">{t("database.editor.empty")}</EmptyState>
      ) : (
        // Monaco defaults to height:100%, which only resolves against a parent
        // with a definite height. Give it a flex-sized box so the editor fills
        // its split pane instead of collapsing to zero height.
        <div className="min-h-0 flex-1">
          <Editor
            defaultLanguage="sql"
            height="100%"
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
        </div>
      )}
      <Dialog onOpenChange={(open) => !savedSql.savePending && setSaveDialogOpen(open)} open={saveDialogOpen}>
        <DialogContent title={t("database.saved.saveTitle")}>
          <DialogHeader>
            <DialogTitle>{t("database.saved.saveTitle")}</DialogTitle>
          </DialogHeader>
          <form onSubmit={submitSavedSql}>
            <DialogBody className="space-y-3">
              <label className="block space-y-1">
                <span className="text-[11px] font-medium uppercase text-[var(--u-color-text-soft)]">
                  {t("database.saved.name")}
                </span>
                <Input
                  autoFocus
                  onChange={(event) => setSnippetName(event.target.value)}
                  placeholder={t("database.saved.namePlaceholder")}
                  value={snippetName}
                />
              </label>
              <div className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-2">
                <div className="mb-1 truncate text-[11px] text-[var(--u-color-text-soft)]">
                  {selectedConnection
                    ? t("database.saved.connectionScope", { name: selectedConnection.name })
                    : t("database.saved.allConnections")}
                </div>
                <pre className="max-h-28 overflow-auto whitespace-pre-wrap break-words font-mono text-[12px] text-[var(--u-color-text-muted)]">
                  {sql.trim()}
                </pre>
              </div>
              {savedSql.error ? (
                <ErrorState className="min-h-[48px]">{formatDatabaseError(savedSql.error)}</ErrorState>
              ) : null}
            </DialogBody>
            <DialogFooter>
              <Button onClick={() => setSaveDialogOpen(false)} size="sm" type="button" variant="ghost">
                {t("common.confirm.cancel")}
              </Button>
              <Button disabled={!snippetName.trim() || !sql.trim() || savedSql.savePending} size="sm" type="submit">
                <Save size={13} />
                {savedSql.savePending ? t("database.saved.saving") : t("common.actions.save")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog onOpenChange={setSavedDialogOpen} open={savedDialogOpen}>
        <DialogContent className="w-[min(680px,calc(100vw-32px))]" title={t("database.saved.listTitle")}>
          <DialogHeader>
            <DialogTitle>{t("database.saved.listTitle")}</DialogTitle>
          </DialogHeader>
          <DialogBody className="p-0">
            {savedSql.isLoading ? (
              <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
                {t("common.state.loading")}
              </div>
            ) : savedSql.saved.length ? (
              <div className="max-h-[420px] overflow-auto">
                {savedSql.saved.map((item) => (
                  <div
                    className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 border-b border-[var(--u-color-border)] px-2 py-1.5 last:border-b-0 hover:bg-[var(--u-color-surface-hover)]"
                    key={item.id}
                  >
                    <button
                      className="min-w-0 cursor-pointer text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color:color-mix(in_srgb,var(--u-color-focus)_32%,transparent)]"
                      onClick={() => loadSavedSql(item)}
                      title={item.sql}
                      type="button"
                    >
                      <span className="block truncate text-[12px] font-semibold text-[var(--u-color-text)]">
                        {item.name}
                      </span>
                      <span className="block truncate text-[11px] text-[var(--u-color-text-soft)]">
                        {savedSqlConnectionLabel(item, connections, t("database.saved.allConnections"), t("database.saved.missingConnection"))}
                        {" · "}
                        {formatSavedSqlTime(item.updatedAt)}
                      </span>
                      <span className="mt-0.5 block truncate font-mono text-[12px] text-[var(--u-color-text-muted)]">
                        {oneLineSql(item.sql)}
                      </span>
                    </button>
                    <IconButton
                      disabled={savedSql.removePending}
                      label={t("database.saved.deleteLabel", { name: item.name })}
                      onClick={() => deleteSavedSql(item)}
                      size="compact"
                    >
                      <Trash2 size={13} />
                    </IconButton>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState className="m-2 min-h-[120px]">{t("database.saved.empty")}</EmptyState>
            )}
            {savedSql.error ? (
              <ErrorState className="m-2 min-h-[48px]">{formatDatabaseError(savedSql.error)}</ErrorState>
            ) : null}
          </DialogBody>
          <DialogFooter>
            <Button onClick={() => setSavedDialogOpen(false)} size="sm" type="button" variant="ghost">
              {t("common.confirm.cancel")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function connectionContext(connection: DatabaseConnection) {
  const database = connection.database ?? connection.sqlitePath ?? "default";
  return `${connection.driver} / ${database}`;
}

function defaultSavedSqlName(sql: string, fallback: string) {
  const firstLine = oneLineSql(sql).replace(/;$/, "");
  if (!firstLine) {
    return fallback;
  }
  return firstLine.length > 80 ? `${firstLine.slice(0, 77)}...` : firstLine;
}

function oneLineSql(sql: string) {
  return sql.trim().replace(/\s+/g, " ");
}

function savedSqlConnectionLabel(
  item: SavedSql,
  connections: DatabaseConnection[],
  allConnections: string,
  missingConnection: string,
) {
  if (!item.connectionId) {
    return allConnections;
  }
  return connections.find((connection) => connection.id === item.connectionId)?.name ?? missingConnection;
}

function formatSavedSqlTime(value: string) {
  return new Date(value).toLocaleString([], {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "2-digit",
  });
}
