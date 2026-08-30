import { useEffect } from "react";
import type { DatabaseTable } from "@unfour/command-client";
import type { DatabaseWorkspaceTab } from "../model/types";
import type { DatabaseTreeModel } from "../model/database-tree";
import { normalizeQueryContext } from "../model/database-query-context";
import type { useDatabaseTabs } from "./useDatabaseTabs";

export function useDatabaseTabSelection(
  activeTab: DatabaseWorkspaceTab | null,
  setSelectedConnection: (id: string | null) => void,
  setSelectedTable: (table: DatabaseTable | null) => void,
) {
  const tabId = activeTab?.id;
  const connectionId = activeTab?.connectionId;
  const table = activeTab?.kind === "table" ? activeTab.table : null;
  useEffect(() => {
    if (!tabId) return;
    setSelectedConnection(connectionId ?? null);
    setSelectedTable(table);
  }, [tabId, connectionId, table, setSelectedConnection, setSelectedTable]);
}

export function useDatabaseQueryContext(
  activeTab: DatabaseWorkspaceTab | null,
  treeModel: DatabaseTreeModel | null,
  defaultDatabase: string | null | undefined,
  updateQueryTab: ReturnType<typeof useDatabaseTabs>["updateQueryTab"],
) {
  const query = activeTab?.kind === "query" ? activeTab : null;
  const id = query?.id;
  const catalog = query?.catalog ?? null;
  const schema = query?.schema ?? null;
  // SQL edits and query results must not drive tree selection or normalization.
  useEffect(() => {
    if (!treeModel || !id) return;
    const next = normalizeQueryContext({ catalog, schema }, treeModel, defaultDatabase);
    if (next.catalog !== catalog || next.schema !== schema) updateQueryTab(id, next);
  }, [catalog, defaultDatabase, id, schema, treeModel, updateQueryTab]);
}
