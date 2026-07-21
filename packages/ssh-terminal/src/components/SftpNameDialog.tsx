import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  useI18n,
} from "@unfour/ui";

export function SftpNameDialog({
  confirmLabel,
  confirmDisabled,
  description,
  error,
  label,
  onConfirm,
  onOpenChange,
  onValueChange,
  open,
  pending,
  title,
  value,
  warning,
}: {
  confirmLabel: string;
  confirmDisabled?: boolean;
  description?: string;
  error?: string | null;
  label: string;
  onConfirm: () => void;
  onOpenChange: (open: boolean) => void;
  onValueChange: (value: string) => void;
  open: boolean;
  pending?: boolean;
  title: string;
  value: string;
  warning?: string | null;
}) {
  const { t } = useI18n();
  return (
    <Dialog onOpenChange={(next) => !pending && onOpenChange(next)} open={open}>
      <DialogContent className="w-[min(420px,calc(100vw-32px))]" title={title}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          {description ? <DialogDescription className="mb-3">{description}</DialogDescription> : null}
          <form
            className="space-y-2"
            onSubmit={(event) => {
              event.preventDefault();
              if (value.trim() && !pending && !confirmDisabled) onConfirm();
            }}
          >
            <label className="block space-y-1">
              <span className="text-[12px] font-medium text-[var(--u-color-text-muted)]">
                {label}
              </span>
              <Input
                autoFocus
                disabled={pending}
                onChange={(event) => onValueChange(event.target.value)}
                value={value}
              />
            </label>
            {warning ? (
              <div className="text-[12px] text-[var(--u-color-warning)]">{warning}</div>
            ) : null}
            {error ? (
              <div className="text-[12px] text-[var(--u-color-danger)]">{error}</div>
            ) : null}
          </form>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button disabled={pending} type="button" variant="ghost">
              {t("common.confirm.cancel")}
            </Button>
          </DialogClose>
          <Button
            disabled={pending || confirmDisabled || !value.trim()}
            onClick={onConfirm}
            type="button"
          >
            {confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
