import { MoreHorizontal } from "lucide-react";
import {
  ContextMenuItem,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@unfour/ui";
import type { ApiSavedRequest } from "@unfour/command-client";
import type { ApiOpenIntent } from "../model/types";

type Translate = (key: string, params?: Record<string, string | number>) => string;

export type RequestTreeActionContext = {
  duplicate: (requestId: string) => void;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  remove: (requestId: string) => void;
  rename: (request: ApiSavedRequest) => void;
  t: Translate;
};

type RequestTreeAction = {
  id: string;
  label: string;
  onSelect: () => void;
  tone?: "danger";
};

function requestTreeActions(
  request: ApiSavedRequest,
  ctx: RequestTreeActionContext,
): RequestTreeAction[] {
  const open = (action: "open" | "send" = "open") =>
    ctx.onOpenIntent({
      action,
      kind: "saved",
      nonce: Date.now(),
      requestId: request.id,
    });

  return [
    {
      id: "open",
      label: ctx.t("api.request.openInTab"),
      onSelect: () => open(),
    },
    {
      id: "send",
      label: ctx.t("api.actions.send"),
      onSelect: () => open("send"),
    },
    {
      id: "rename",
      label: ctx.t("api.request.rename"),
      onSelect: () => ctx.rename(request),
    },
    {
      id: "duplicate",
      label: ctx.t("api.actions.duplicate"),
      onSelect: () => ctx.duplicate(request.id),
    },
    {
      id: "copy-url",
      label: ctx.t("api.request.copyUrl"),
      onSelect: () => void navigator.clipboard?.writeText(request.url),
    },
    {
      id: "delete",
      label: ctx.t("api.actions.deleteRequest"),
      onSelect: () => ctx.remove(request.id),
      tone: "danger",
    },
  ];
}

export function RequestContextMenu({
  ctx,
  request,
}: {
  ctx: RequestTreeActionContext;
  request: ApiSavedRequest;
}) {
  return (
    <>
      {requestTreeActions(request, ctx).map((action) => (
        <ContextMenuItem
          key={action.id}
          onSelect={action.onSelect}
          tone={action.tone}
        >
          {action.label}
        </ContextMenuItem>
      ))}
    </>
  );
}

export function RequestActionMenu({
  ctx,
  request,
}: {
  ctx: RequestTreeActionContext;
  request: ApiSavedRequest;
}) {
  const label = ctx.t("api.request.actionsLabel", { name: request.name });
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          aria-label={label}
          className="grid h-5 w-5 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:opacity-100 group-hover:opacity-100 data-[state=open]:opacity-100"
          onClick={(event) => event.stopPropagation()}
          onPointerDown={(event) => event.stopPropagation()}
          title={label}
          type="button"
        >
          <MoreHorizontal size={13} />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {requestTreeActions(request, ctx).map((action) => (
          <DropdownMenuItem
            className={action.tone === "danger" ? "text-[var(--u-color-danger)]" : undefined}
            key={action.id}
            onSelect={action.onSelect}
          >
            {action.label}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
