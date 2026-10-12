import { Columns3, Database, Eye, FileText, RefreshCw, Table2 } from "lucide-react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable, SavedSql } from "@unfour/command-client";
import { Badge, ContextMenuItem, IconButton, type TreeViewItem, type useI18n } from "@unfour/ui";
import { buildDatabaseTree, databaseTableTreeId } from "../model/database-tree";
import { SavedSqlContextMenu, TableContextMenu } from "./database-tree-menus";

// One render owns both the visible nodes and their routing/expansion index.
// Builders never retain these maps across renders or trigger schema requests.
export function createDatabaseTreeIndex() {
  return {
    tableLookup: new Map<string, { connectionId: string; table: DatabaseTable }>(),
    catalogLookup: new Map<string, { connectionId: string; catalog: string }>(),
    savedSqlLookup: new Map<string, SavedSql>(),
    defaultExpandedIds: new Set<string>(),
  };
}

type ContentContext = {
  connection: DatabaseConnection;
  index: ReturnType<typeof createDatabaseTreeIndex>;
  t: ReturnType<typeof useI18n>["t"];
  onDeleteSavedSql?: (item: SavedSql) => void;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onExportTable?: (connection: DatabaseConnection, table: DatabaseTable) => void;
  onNewQuery?: (connection?: DatabaseConnection, catalog?: string) => void;
  onOpenSavedSql?: (item: SavedSql) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
};

type ConnectionContentOptions = ContentContext & {
  catalogNames?: string[];
  loadErrors?: Record<string, string>;
  loadingKeys?: string[];
  savedSql?: SavedSql[];
  schemaCache?: Record<string, DatabaseSchema>;
};

/** Build descendants only for connected roots; the component owns that gate. */
export function buildConnectionChildren(options: ConnectionContentOptions): TreeViewItem[] {
  const { connection, index, t, catalogNames, loadErrors, loadingKeys, savedSql, schemaCache, onNewQuery } = options;
  const isLoading = (key: string) => loadingKeys?.includes(key) ?? false;
  const placeholder = (key: string) => statusChild(key, isLoading(key), loadErrors?.[key], t);
  // Unresolved legacy snippets remain accessible in the Saved SQL dialog.
  const savedGroup = (catalog: string | null, parentId: string) => buildSavedSqlGroup(
    options, parentId, catalog === null ? savedSql : savedSql?.filter((item) => item.catalog === catalog),
  );

  // SQLite has no catalog level. Saved SQL appears after its schema loads.
  if (connection.driver === "sqlite") {
    const key = `${connection.id}::`;
    const schema = schemaCache?.[key];
    if (!schema) return [placeholder(key)];
    const contents = renderCatalogContents(options, connection.id, schema.tables);
    const group = savedGroup(null, connection.id);
    return group ? [...contents, group] : contents;
  }

  // PostgreSQL / MySQL catalogs load on expand; their saved SQL is independent
  // of schema loading and remains available alongside a loading/error child.
  const namesKey = `names::${connection.id}`;
  if (!catalogNames) return [placeholder(namesKey)];
  if (!catalogNames.length) {
    return [{ disabled: true, id: `${connection.id}:no-databases`, label: t("database.tree.noDatabases") }];
  }
  const connectedCatalog = connection.database?.trim() || null;
  return catalogNames.map((name) => {
    const id = `${connection.id}:catalog:${name}`;
    index.catalogLookup.set(id, { connectionId: connection.id, catalog: name });
    const key = `${connection.id}::${name}`;
    const schema = schemaCache?.[key];
    const children = schema ? renderCatalogContents(options, id, schema.tables) : [placeholder(key)];
    if (schema && name === connectedCatalog) index.defaultExpandedIds.add(id);
    const group = savedGroup(name, id);
    if (group) children.push(group);
    return {
      children,
      contextMenu: onNewQuery ? <ContextMenuItem onSelect={() => onNewQuery(connection, name)}>{t("database.actions.newQuery")}</ContextMenuItem> : undefined,
      icon: <Database size={13} />,
      id,
      label: name,
      loading: isLoading(key),
      title: name,
    };
  });
}

function buildSavedSqlGroup(
  { connection, index, t, onDeleteSavedSql, onOpenSavedSql }: ContentContext,
  parentId: string,
  savedSql: SavedSql[] | undefined,
): TreeViewItem | null {
  if (!onOpenSavedSql && !onDeleteSavedSql) return null;
  const items = savedSql ?? [];
  const groupId = `${parentId}:saved-sql`;
  const children = items.length
    ? items.map((item) => {
        const id = `${connection.id}:saved-sql:${item.id}`;
        index.savedSqlLookup.set(id, item);
        return {
          contextMenu: <SavedSqlContextMenu item={item} onDelete={onDeleteSavedSql} onOpen={onOpenSavedSql} t={t} />,
          icon: <FileText size={13} />,
          id,
          label: item.name,
          title: item.sql,
        };
      })
    : [{ disabled: true, id: `${groupId}:empty`, label: t("database.saved.empty") }];
  return {
    children,
    icon: <FileText size={13} />,
    id: groupId,
    label: t("database.tree.savedQueriesGroup"),
    meta: <Badge tone="neutral">{items.length}</Badge>,
  };
}

function statusChild(
  key: string,
  loading: boolean,
  error: string | undefined,
  t: ContentContext["t"],
): TreeViewItem {
  if (error) return { disabled: true, id: `${key}:error`, label: error, title: error };
  return {
    disabled: true,
    id: `${key}:${loading ? "loading" : "placeholder"}`,
    label: loading ? t("database.tree.loadingSchema") : t("database.tree.expandToLoad"),
  };
}

// PostgreSQL nests schemas; MySQL/SQLite list object groups directly.
function renderCatalogContents(context: ContentContext, parentId: string, tables: DatabaseTable[]): TreeViewItem[] {
  const { connection, index, onRefreshSchema } = context;
  if (!tables.length) {
    return [{ disabled: true, id: `${parentId}:no-tables`, label: "No tables or views found" }];
  }
  const catalog = buildDatabaseTree(tables).catalogs[0];
  if (!catalog || !catalog.hasSchemaLevel) return buildTableGroups(context, parentId, tables);
  return catalog.schemas.map((schemaNode) => {
    const id = `${parentId}:schema:${schemaNode.key}`;
    index.defaultExpandedIds.add(id);
    return {
      actions: onRefreshSchema ? (
        <IconButton label={`Refresh ${schemaNode.key} schema`} onClick={() => onRefreshSchema(connection)} size="compact">
          <RefreshCw size={12} />
        </IconButton>
      ) : undefined,
      children: buildTableGroups(context, id, schemaNode.tables),
      icon: <Columns3 size={13} />,
      id,
      label: schemaNode.key,
      meta: <Badge tone="neutral">{schemaNode.tables.length}</Badge>,
      title: schemaNode.key,
    };
  });
}

function buildTableGroups(context: ContentContext, parentId: string, tables: DatabaseTable[]): TreeViewItem[] {
  const { index, t } = context;
  const baseTables = tables.filter((table) => !isViewKind(table.kind));
  const views = tables.filter((table) => isViewKind(table.kind));
  const groups: TreeViewItem[] = [];
  if (baseTables.length) {
    const id = `${parentId}:tables`;
    index.defaultExpandedIds.add(id);
    groups.push({
      children: baseTables.map((table) => tableItem(context, table)),
      icon: <Table2 size={13} />,
      id,
      label: t("database.tree.tablesGroup"),
      meta: <Badge tone="teal">{baseTables.length}</Badge>,
    });
  }
  if (views.length) {
    groups.push({
      children: views.map((table) => tableItem(context, table)),
      icon: <Eye size={13} />,
      id: `${parentId}:views`,
      label: t("database.tree.viewsGroup"),
      meta: <Badge tone="neutral">{views.length}</Badge>,
    });
  }
  return groups;
}

function isViewKind(kind: string) {
  return kind.toLowerCase().includes("view");
}

function tableItem(
  { connection, index, t, onDesignTable, onExportTable, onPreviewTable, onUseSql }: ContentContext,
  table: DatabaseTable,
): TreeViewItem {
  const id = databaseTableTreeId(connection.id, table);
  index.tableLookup.set(id, { connectionId: connection.id, table });
  return {
    contextMenu: (
      <TableContextMenu
        connection={connection}
        onDesignTable={onDesignTable}
        onExportTable={onExportTable}
        onPreviewTable={onPreviewTable}
        onUseSql={onUseSql}
        t={t}
        table={table}
      />
    ),
    icon: <Table2 size={13} />,
    id,
    label: table.name,
    title: [table.catalog, table.schema, table.name].filter(Boolean).join("."),
  };
}
