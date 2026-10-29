import { useRef } from "react";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  useI18n,
} from "@unfour/ui";

export function ApiCloseRequestDialog({
  onCancel,
  onDiscard,
  onSave,
  onRestoreFocus,
  open,
  title,
}: {
  onCancel: () => void;
  onDiscard: () => void;
  onSave: () => void;
  onRestoreFocus: () => void;
  open: boolean;
  title: string;
}) {
  const { t } = useI18n();
  const returnFocusRef = useRef<HTMLElement | null>(null);
  return (
    <Dialog onOpenChange={(next) => !next && onCancel()} open={open}>
      <DialogContent
        onOpenAutoFocus={() => {
          returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          // A save dialog may have replaced this confirmation. Keep its focus.
          if (document.querySelector('[role="dialog"][data-state="open"]')) return;
          if (returnFocusRef.current?.isConnected && !returnFocusRef.current.closest('[role="menu"]')) {
            returnFocusRef.current.focus();
          } else {
            onRestoreFocus();
          }
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("api.close.title")}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          <DialogDescription>
            {t("api.close.description", { title })}
          </DialogDescription>
        </DialogBody>
        <DialogFooter>
          <Button onClick={onCancel} type="button" variant="ghost">
            {t("common.confirm.cancel")}
          </Button>
          <Button onClick={onDiscard} type="button" variant="outline">
            {t("api.close.discard")}
          </Button>
          <Button onClick={onSave} type="button">
            {t("common.actions.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
