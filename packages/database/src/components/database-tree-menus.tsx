import { Copy, CopyPlus, MoreVertical, Pencil, Play, PlusCircle, RefreshCw, Square, Trash2 } from "lucide-react";
import { getDatabaseTableStructure, type DatabaseConnection, type DatabaseTable, type SavedSql } from "@unfour/command-client";
import {
  ContextMenuItem,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";
import type { DatabaseConnectionStatus } from "../model/types";


async function copyToClipboard(value: string, onError: (error: unknown) => void) {
  try {
    if (!navigator.clipboard) {
      throw new Error("Clipboard API is unavailable in this context");
    }
    await navigator.clipboard.writeText(value);
  } catch (error) {
    onError(error);
  }
}

export function SavedSqlContextMenu({
  item,
  onDelete,
  onOpen,
  t,
}: {
  item: SavedSql;
  onDelete?: (item: SavedSql) => void;
  onOpen?: (item: SavedSql) => void;
  t: ReturnType<typeof useI18n>["t"];
}) {
  const handleError = useFeedbackErrorHandler();
  return (
    <>
      {onOpen && (
        <ContextMenuItem onSelect={() => onOpen(item)}>
          <Play size={13} />
          {t("database.tree.openSavedSql")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void copyToClipboard(item.sql, handleError)}>
        <Copy size={13} />
        {t("database.tree.copySavedSql")}
      </ContextMenuItem>
      {onDelete && (
        <ContextMenuItem onSelect={() => onDelete(item)} tone="danger">
          <Trash2 size={13} />
          {t("database.tree.deleteSavedSql")}
        </ContextMenuItem>
      )}
    </>
  );
}

export function TableContextMenu({
  connection,
  onExportTable,
  onDesignTable,
  onPreviewTable,
  onUseSql,
  t,
  table,
}: {
  connection: DatabaseConnection;
  onExportTable?: (connection: DatabaseConnection, table: DatabaseTable) => void;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  t: ReturnType<typeof useI18n>["t"];
  table: DatabaseTable;
}) {
  const handleError = useFeedbackErrorHandler();
  return (
    <>
      {onPreviewTable && (
        <ContextMenuItem onSelect={() => onPreviewTable(connection.id, table)}>
          {t("database.tree.previewData")}
        </ContextMenuItem>
      )}
      {onDesignTable && (
        <ContextMenuItem onSelect={() => onDesignTable(connection.id, table)}>
          {t("database.tree.designTable")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void getDatabaseTableStructure({ workspaceId: connection.workspaceId, connectionId: connection.id, catalog: table.catalog, schema: table.schema, tableName: table.name }).then((structure) => {
        if (structure.ddl) return copyToClipboard(structure.ddl, handleError);
        throw new Error("DDL is unavailable");
      }).catch(handleError)}>
        <Copy size={13} />{t("database.export.copyDdl")}
      </ContextMenuItem>
      {onExportTable && table.kind === "table" ? <ContextMenuItem onSelect={() => onExportTable(connection, table)}>{t("database.export.exportTable")}</ContextMenuItem> : null}
      {onUseSql && (
        <ContextMenuItem onSelect={() => onUseSql(connection.id, generateSelectSql(connection.driver, table), table)}>
          {t("database.tree.generateSelect")}
        </ContextMenuItem>
      )}
      {/* Views are read-only: hide "Generate INSERT" to avoid offering an
          action that will fail at execution time. Tables keep the full menu. */}
      {onUseSql && table.kind !== "view" && (
        <ContextMenuItem onSelect={() => onUseSql(connection.id, generateInsertSql(connection.driver, table), table)}>
          {t("database.tree.generateInsert")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void copyToClipboard(table.name, handleError)}>
        {t("database.tree.copyTableName")}
      </ContextMenuItem>
    </>
  );
}

function quoteDbIdentifier(driver: string, value: string) {
  if (driver === "mysql") {
    return `\`${value.split("`").join("``")}\``;
  }
  return `"${value.split('"').join('""')}"`;
}

function qualifiedSqlName(driver: string, table: DatabaseTable) {
  const name = quoteDbIdentifier(driver, table.name);
  // PostgreSQL qualifies by schema; MySQL qualifies by its database (catalog).
  const qualifier = table.schema ?? table.catalog;
  return qualifier ? `${quoteDbIdentifier(driver, qualifier)}.${name}` : name;
}

function generateSelectSql(driver: string, table: DatabaseTable) {
  const columns = table.columns.length
    ? table.columns.map((column) => quoteDbIdentifier(driver, column.name)).join(", ")
    : "*";
  return `SELECT ${columns}\nFROM ${qualifiedSqlName(driver, table)}\nLIMIT 100;`;
}

function generateInsertSql(driver: string, table: DatabaseTable) {
  if (!table.columns.length) {
    return `INSERT INTO ${qualifiedSqlName(driver, table)} () VALUES ();`;
  }
  const columns = table.columns.map((column) => quoteDbIdentifier(driver, column.name)).join(", ");
  const placeholders = table.columns.map(() => "NULL").join(", ");
  return `INSERT INTO ${qualifiedSqlName(driver, table)} (${columns})\nVALUES (${placeholders});`;
}

export function ConnectionContextMenu({
  connection,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onDuplicateConnection,
  onEditConnection,
  onNewQuery,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onDuplicateConnection?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();

  return (
    <>
      <ContextMenuItem onSelect={() => onConnect?.(connection)}>
        <Play size={13} />
        {t("common.actions.connect")}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={status === "disconnected"}
        onSelect={() => onDisconnect?.(connection)}
      >
        <Square size={13} />
        {t("common.actions.disconnect")}
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => onNewQuery?.(connection)}>
        <PlusCircle size={13} />
        {t("database.actions.newQuery")}
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => onRefreshSchema?.(connection)}>
        <RefreshCw size={13} />
        {t("database.actions.refreshSchema")}
      </ContextMenuItem>
      {onEditConnection && (
        <ContextMenuItem onSelect={() => onEditConnection(connection)}>
          <Pencil size={13} />
          {t("database.tree.editConnection")}
        </ContextMenuItem>
      )}
      {onDuplicateConnection && (
        <ContextMenuItem onSelect={() => onDuplicateConnection(connection)}>
          <CopyPlus size={13} />
          {t("database.tree.duplicateConnection")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void copyToClipboard(connection.name, handleError)}>
        <Copy size={13} />
        {t("database.tree.copyName")}
      </ContextMenuItem>
      {onDeleteConnection && (
        <ContextMenuItem onSelect={() => onDeleteConnection(connection)} tone="danger">
          <Trash2 size={13} />
          {t("database.tree.deleteConnection")}
        </ContextMenuItem>
      )}
    </>
  );
}

// Right-aligned row menu (the "⋯" button) — kept in sync with
// ConnectionContextMenu so the inline menu and the right-click menu expose the
// same actions. Item order matches the context menu for consistency.
export function ConnectionRowMenu({
  connection,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onDuplicateConnection,
  onEditConnection,
  onNewQuery,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onDuplicateConnection?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton
          disableTooltip
          label={t("database.tree.actionsLabel", { name: connection.name })}
          size="compact"
        >
          <MoreVertical size={14} />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => onConnect?.(connection)}>
          <Play size={13} />
          {t("common.actions.connect")}
        </DropdownMenuItem>
        <DropdownMenuItem disabled={status === "disconnected"} onSelect={() => onDisconnect?.(connection)}>
          <Square size={13} />
          {t("common.actions.disconnect")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onNewQuery?.(connection)}>
          <PlusCircle size={13} />
          {t("database.actions.newQuery")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onRefreshSchema?.(connection)}>
          <RefreshCw size={13} />
          {t("database.actions.refreshSchema")}
        </DropdownMenuItem>
        {onEditConnection && (
          <DropdownMenuItem onSelect={() => onEditConnection(connection)}>
            <Pencil size={13} />
            {t("database.tree.editConnection")}
          </DropdownMenuItem>
        )}
        {onDuplicateConnection && (
          <DropdownMenuItem onSelect={() => onDuplicateConnection(connection)}>
            <CopyPlus size={13} />
            {t("database.tree.duplicateConnection")}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={() => void copyToClipboard(connection.name, handleError)}>
          <Copy size={13} />
          {t("database.tree.copyName")}
        </DropdownMenuItem>
        {onDeleteConnection && (
          <DropdownMenuItem
            className="text-[var(--u-color-danger)] data-[highlighted]:bg-[var(--u-color-danger-soft)]"
            onSelect={() => onDeleteConnection(connection)}
          >
            <Trash2 size={13} />
            {t("database.tree.deleteConnection")}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
