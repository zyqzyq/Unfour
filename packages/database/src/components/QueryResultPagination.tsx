import { ChevronLeft, ChevronRight } from "lucide-react";
import { IconButton, useI18n } from "@unfour/ui";

export function QueryResultPagination({ count, onChange, page, pageSize }: {
  count: number;
  onChange: (page: number) => void;
  page: number;
  pageSize: number;
}) {
  const { t } = useI18n();
  const offset = page * pageSize;
  const pages = Math.ceil(count / pageSize);
  return (
    <div className="flex shrink-0 items-center gap-1">
      <span className="mr-2">{t("database.grid.rowRangeTotal", {
        first: offset + 1, last: Math.min(count, offset + pageSize), total: count,
      })}</span>
      <IconButton disabled={page === 0} label={t("database.grid.prevPage")} onClick={() => onChange(page - 1)} size="compact">
        <ChevronLeft size={14} />
      </IconButton>
      <span>{t("database.grid.page", { page: page + 1, pages })}</span>
      <IconButton disabled={page + 1 === pages} label={t("database.grid.nextPage")} onClick={() => onChange(page + 1)} size="compact">
        <ChevronRight size={14} />
      </IconButton>
    </div>
  );
}
