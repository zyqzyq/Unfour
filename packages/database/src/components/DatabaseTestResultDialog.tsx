import { CheckCircle2, XCircle } from "lucide-react";
import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  useI18n,
} from "@unfour/ui";
import type { DatabaseTestResult } from "@unfour/command-client";

/**
 * Popup that surfaces the outcome of a "test connection" attempt. The inline
 * footer box in the connection dialog could not show long failure details, so
 * the full message is rendered here in a scrollable, wrapping body. Mirrors the
 * SSH module's SshTestResultDialog.
 */
export function DatabaseTestResultDialog({
  onOpenChange,
  result,
}: {
  onOpenChange: (open: boolean) => void;
  result: DatabaseTestResult | null;
}) {
  const { t } = useI18n();
  const title = result?.ok ? t("database.connection.testSuccess") : t("database.connection.testFailed");

  return (
    <Dialog onOpenChange={onOpenChange} open={result !== null}>
      <DialogContent className="w-[min(460px,calc(100vw-32px))]" title={title}>
        <DialogHeader>
          <DialogTitle>
            <span
              className="flex items-center gap-2"
              style={{
                color: result?.ok
                  ? "var(--u-color-success)"
                  : "var(--u-color-danger)",
              }}
            >
              {result?.ok ? <CheckCircle2 size={16} /> : <XCircle size={16} />}
              {title}
            </span>
          </DialogTitle>
        </DialogHeader>
        <DialogBody>
          <p className="max-h-[40vh] overflow-auto whitespace-pre-wrap break-words text-[12.5px] leading-relaxed text-[var(--u-color-text)]">
            {result?.message}
          </p>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button autoFocus type="button">
              {t("database.connection.testClose")}
            </Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
