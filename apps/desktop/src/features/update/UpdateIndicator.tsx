import { Badge, Button, useI18n, type TFunction } from "@unfour/ui";
import { useUpdate } from "./useUpdate";
import type { UpdateState } from "./updateTypes";

function indicatorLabel(state: UpdateState, t: TFunction) {
  switch (state.kind) {
    case "available": return t("updates.indicator");
    case "installing": return t("updates.startingInstaller");
    case "installed": return t("updates.restartOptions");
    case "downloading":
      return state.total && state.total > 0
        ? t("updates.downloading", { percent: Math.min(100, Math.round(state.downloaded / state.total * 100)) })
        : t("updates.downloadingUnknown");
    default: return null;
  }
}

export function UpdateIndicator() {
  const { t } = useI18n();
  const { state, openDialog } = useUpdate();
  const label = indicatorLabel(state, t);
  if (!label) return null;
  return (
    <Button className="h-7 max-w-44 px-2" onClick={openDialog} size="sm" type="button" variant="ghost">
      <Badge className="truncate" tone="teal">{label}</Badge>
    </Button>
  );
}
