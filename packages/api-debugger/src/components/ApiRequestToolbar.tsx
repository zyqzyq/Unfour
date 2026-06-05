import { Braces, Plus, Send } from "lucide-react";
import { Badge, Button, IconButton } from "@unfour/ui";
import type { ApiRequestInput } from "@unfour/command-client";
import {
  apiRequestStateLabel,
  apiRequestStateTone,
} from "../model/api-request-state";
import type { ApiRequestState } from "../model/types";
import { RequestActionsMenu } from "./RequestActionsMenu";

export function ApiRequestToolbar({
  canDelete,
  canDuplicate,
  canExport,
  collectionStatus,
  deleting,
  duplicating,
  importing,
  input,
  onDelete,
  onDuplicate,
  onExport,
  onImport,
  onNewRequest,
  onSave,
  requestState,
  saving,
  selectedUrl,
  sending,
  title,
}: {
  canDelete: boolean;
  canDuplicate: boolean;
  canExport: boolean;
  collectionStatus: string;
  deleting: boolean;
  duplicating: boolean;
  importing: boolean;
  input: ApiRequestInput;
  onDelete: () => void;
  onDuplicate: () => void;
  onExport: () => void;
  onImport: () => void;
  onNewRequest: () => void;
  onSave: () => void;
  requestState: ApiRequestState;
  saving: boolean;
  selectedUrl: string;
  sending: boolean;
  title: string;
}) {
  return (
    <div className="flex h-[var(--u-size-section-toolbar)] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <Braces className="shrink-0 text-[var(--u-color-text-muted)]" size={15} />
        <div className="min-w-0">
          <div className="truncate text-[13px] font-semibold text-[var(--u-color-text)]">
            {title}
          </div>
          <div className="truncate text-[12px] text-[var(--u-color-text-muted)]">
            {selectedUrl || "Select a collection item or draft a new request"}
          </div>
        </div>
        <Badge tone={apiRequestStateTone[requestState]}>
          {apiRequestStateLabel[requestState]}
        </Badge>
        {collectionStatus && (
          <span className="hidden truncate text-[12px] text-[var(--u-color-text-muted)] lg:inline">
            {collectionStatus}
          </span>
        )}
      </div>
      <IconButton label="New request" onClick={onNewRequest} tooltip="New request">
        <Plus size={15} />
      </IconButton>
      <RequestActionsMenu
        canDelete={canDelete}
        canDuplicate={canDuplicate}
        canExport={canExport}
        deleting={deleting}
        duplicating={duplicating}
        importing={importing}
        onDelete={onDelete}
        onDuplicate={onDuplicate}
        onExport={onExport}
        onImport={onImport}
        onSave={onSave}
        saving={saving}
      />
      <Button disabled={sending || !input.url.trim()} type="submit">
        <Send size={15} />
        {sending ? "Sending" : "Send"}
      </Button>
    </div>
  );
}
