import * as React from "react";
import { Button } from "./button";
import {
  Dialog,
  DialogClose,
} from "./dialog-primitives";
import {
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./dialog";
import { useI18n } from "./i18n";

/**
 * Shared confirmation dialog for destructive or irreversible actions.
 *
 * Replaces native `window.confirm`, which renders outside the app styling,
 * cannot tone the confirm button, and always focuses the confirm action.
 * Per the interaction guidelines, the Cancel button receives initial focus so
 * pressing Enter does not accidentally confirm a high-impact action.
 */
export function ConfirmDialog({
  cancelLabel,
  confirmLabel,
  description,
  onConfirm,
  onOpenChange,
  open,
  pending = false,
  title,
  tone = "danger",
}: {
  cancelLabel?: string;
  confirmLabel: string;
  description?: React.ReactNode;
  onConfirm: () => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  pending?: boolean;
  title: string;
  tone?: "default" | "danger";
}) {
  const { t } = useI18n();

  return (
    <Dialog onOpenChange={(next) => !pending && onOpenChange(next)} open={open}>
      <DialogContent className="w-[min(420px,calc(100vw-32px))]" title={title}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        {description && (
          <DialogBody>
            <DialogDescription>{description}</DialogDescription>
          </DialogBody>
        )}
        <DialogFooter>
          <DialogClose asChild>
            <Button autoFocus disabled={pending} type="button" variant="ghost">
              {cancelLabel ?? t("common.confirm.cancel")}
            </Button>
          </DialogClose>
          <Button
            disabled={pending}
            onClick={onConfirm}
            type="button"
            variant={tone === "danger" ? "danger" : "default"}
          >
            {confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
