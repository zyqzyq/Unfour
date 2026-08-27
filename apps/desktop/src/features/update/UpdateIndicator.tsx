import { Badge, Button, useI18n } from "@unfour/ui";
import { useUpdate } from "./useUpdate";

export function UpdateIndicator() {
  const { t } = useI18n();
  const { state, openDialog } = useUpdate();
  if (
    state.kind !== "available"
    && state.kind !== "downloading"
    && state.kind !== "installing"
  ) return null;
  const label = state.kind === "available"
    ? t("updates.indicator")
    : state.kind === "installing"
      ? t("updates.startingInstaller")
      : state.total && state.total > 0
        ? t("updates.downloading", {
            percent: Math.min(100, Math.round((state.downloaded / state.total) * 100)),
          })
        : t("updates.downloadingUnknown");
  return (
    <Button className="h-7 max-w-44 px-2" onClick={openDialog} size="sm" type="button" variant="ghost">
      <Badge className="truncate" tone="teal">{label}</Badge>
    </Button>
  );
}
