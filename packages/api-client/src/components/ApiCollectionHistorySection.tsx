import type { ApiHistoryItem } from "@unfour/command-client";
import { SidebarSection, useI18n } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { ApiHistoryTree } from "./ApiHistoryTree";
import { SidebarEmpty } from "./ApiTreeLabels";

export function ApiCollectionHistorySection({ items, onOpenIntent }: {
  items: ApiHistoryItem[];
  onOpenIntent: (intent: ApiOpenIntent) => void;
}) {
  const { t } = useI18n();
  return (
    <SidebarSection
      className="max-h-[220px] shrink-0 overflow-y-auto border-t border-[var(--u-color-border)] px-2 pb-2 pt-2"
      title={t("api.sidebar.history")}
    >
      {items.length > 0 ? (
        <ApiHistoryTree items={items} onOpenIntent={onOpenIntent} />
      ) : (
        <SidebarEmpty>{t("api.sidebar.historyEmptyCompact")}</SidebarEmpty>
      )}
    </SidebarSection>
  );
}
