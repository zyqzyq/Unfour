import { useState } from "react";
import { defaultDatabaseTabs } from "../model/database-tabs";
import type { DatabaseResultTab } from "../model/types";

export function useDatabaseLayout() {
  const [activeTabId, setActiveTabId] = useState(defaultDatabaseTabs[0].id);
  const [tabs, setTabs] = useState(defaultDatabaseTabs);
  const [resultTab, setResultTab] = useState<DatabaseResultTab>("results");
  const [inspectorTab, setInspectorTab] = useState<"columns" | "indexes" | "constraints" | "properties" | "ddl">("columns");

  return {
    activeTabId,
    inspectorTab,
    resultTab,
    setActiveTabId,
    setInspectorTab,
    setResultTab,
    setTabs,
    tabs,
  };
}
