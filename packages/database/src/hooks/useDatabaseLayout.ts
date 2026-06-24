import { useState } from "react";
import { defaultDatabaseTabs } from "../model/database-tabs";
import type { DatabaseResultTab, DatabaseWorkspaceTabId, TableSegment } from "../model/types";

export function useDatabaseLayout() {
  const [activeTabId, setActiveTabId] = useState<DatabaseWorkspaceTabId>(defaultDatabaseTabs[0].id);
  const [tabs, setTabs] = useState(defaultDatabaseTabs);
  const [tableSegment, setTableSegment] = useState<TableSegment>("data");
  const [resultTab, setResultTab] = useState<DatabaseResultTab>("results");
  const [inspectorTab, setInspectorTab] = useState<"ddl" | "indexes" | "constraints" | "properties">("ddl");

  return {
    activeTabId,
    inspectorTab,
    resultTab,
    setActiveTabId,
    setInspectorTab,
    setResultTab,
    setTableSegment,
    setTabs,
    tableSegment,
    tabs,
  };
}
