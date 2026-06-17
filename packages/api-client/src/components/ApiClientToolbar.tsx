import { Download, Plus, Settings2, Upload } from "lucide-react";
import { Button, Toolbar, ToolbarGroup } from "@unfour/ui";

export function ApiClientToolbar({
  onImport,
  onNewRequest,
}: {
  onImport: () => void;
  onNewRequest: () => void;
}) {
  return (
    <Toolbar className="h-[var(--u-size-section-toolbar)]">
      <ToolbarGroup>
        <Button onClick={onNewRequest} size="sm" type="button" variant="ghost">
          <Plus size={14} />
          New Request
        </Button>
        <Button onClick={onImport} size="sm" type="button" variant="ghost">
          <Upload size={14} />
          Import
        </Button>
        <Button
          disabled
          onClick={() => {}}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Download size={14} />
          Export
        </Button>
      </ToolbarGroup>
      <ToolbarGroup>
        <span className="rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-2 py-1 text-[11px] text-[var(--u-color-text-muted)]">
          No Environment
        </span>
        <Button
          disabled
          onClick={() => {}}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Settings2 size={14} />
        </Button>
      </ToolbarGroup>
    </Toolbar>
  );
}
