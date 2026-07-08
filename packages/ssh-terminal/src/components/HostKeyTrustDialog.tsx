import { Shield, ShieldAlert, ShieldCheck, ShieldX } from "lucide-react";
import type { SshHostFingerprintInfo } from "@unfour/command-client";
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
  DialogXClose,
  useI18n,
} from "@unfour/ui";

export function HostKeyTrustDialog({
  existingFingerprint,
  host,
  mismatchError,
  onConfirm,
  onOpenChange,
  onResetAndReconnect,
  open,
  pending,
  port,
  resetPending,
}: {
  existingFingerprint: SshHostFingerprintInfo | null | undefined;
  host: string;
  mismatchError?: string | null;
  onConfirm: () => void;
  onOpenChange: (open: boolean) => void;
  onResetAndReconnect?: () => void;
  open: boolean;
  pending?: boolean;
  port: number;
  resetPending?: boolean;
}) {
  const { t } = useI18n();
  const isMismatch = Boolean(mismatchError);
  const isFirstTrust = !existingFingerprint && !isMismatch;

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent>
        <DialogHeader>
          <div className="flex items-center gap-2">
            {isMismatch ? (
              <ShieldX className="shrink-0 text-[var(--u-color-danger)]" size={20} />
            ) : isFirstTrust ? (
              <Shield className="shrink-0 text-[var(--u-color-warning,orange)]" size={20} />
            ) : (
              <ShieldCheck className="shrink-0 text-[var(--u-color-success,green)]" size={20} />
            )}
            <div className="min-w-0">
              <DialogTitle>
                {isMismatch
                  ? t("ssh.trust.mismatchTitle")
                  : isFirstTrust
                    ? t("ssh.trust.verifyTitle")
                    : t("ssh.trust.verifiedTitle")}
              </DialogTitle>
              <DialogDescription>
                {host}:{port}
              </DialogDescription>
            </div>
          </div>
          <DialogXClose />
        </DialogHeader>
        <DialogBody className="space-y-3">
          {isMismatch ? (
            <>
              <div className="rounded border border-[var(--u-color-danger,red)] bg-[var(--u-color-danger,red)]/10 p-3">
                <div className="flex items-start gap-2">
                  <ShieldAlert
                    className="mt-0.5 shrink-0 text-[var(--u-color-danger)]"
                    size={16}
                  />
                  <div className="space-y-1 text-[13px]">
                    <p className="font-semibold">{t("ssh.trust.mismatchHeading")}</p>
                    <p className="text-[var(--u-color-text-soft)]">
                      {t("ssh.trust.mismatchBody")}
                    </p>
                  </div>
                </div>
              </div>
              {existingFingerprint && (
                <div className="space-y-1.5 rounded border border-[var(--u-color-border)] p-2">
                  <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
                    {t("ssh.trust.previousFingerprint")}
                  </span>
                  <code className="block break-all text-[12px] text-[var(--u-color-text)]">
                    {existingFingerprint.fingerprint}
                  </code>
                </div>
              )}
              <p className="text-[12px] text-[var(--u-color-text-soft)]">
                {mismatchError}
              </p>
            </>
          ) : isFirstTrust ? (
            <>
              <div className="rounded border border-[var(--u-color-warning,orange)]/30 bg-[var(--u-color-warning,orange)]/5 p-3">
                <div className="flex items-start gap-2">
                  <Shield
                    className="mt-0.5 shrink-0 text-[var(--u-color-warning,orange)]"
                    size={16}
                  />
                  <div className="space-y-1 text-[13px]">
                    <p className="font-semibold">{t("ssh.trust.firstTrustHeading")}</p>
                    <p className="text-[var(--u-color-text-soft)]">
                      {t("ssh.trust.firstTrustBody")}
                    </p>
                  </div>
                </div>
              </div>
              <p className="text-[12px] text-[var(--u-color-text-soft)]">
                {t("ssh.trust.firstTrustQuestion")}
              </p>
            </>
          ) : (
            <>
              <div className="space-y-1.5 rounded border border-[var(--u-color-border)] p-2">
                <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
                  {t("ssh.trust.trustedFingerprint")}
                </span>
                <code className="block break-all text-[12px] text-[var(--u-color-text)]">
                  {existingFingerprint?.fingerprint}
                </code>
                <span className="block text-[11px] text-[var(--u-color-text-soft)]">
                  {t("ssh.trust.trustedSince", {
                    date: existingFingerprint
                      ? new Date(existingFingerprint.createdAt).toLocaleDateString()
                      : t("ssh.trust.trustedSinceUnknown"),
                  })}
                </span>
              </div>
            </>
          )}
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="outline">
              {isMismatch ? t("ssh.trust.close") : t("ssh.trust.cancel")}
            </Button>
          </DialogClose>
          {isMismatch ? (
            <Button
              disabled={resetPending}
              onClick={onResetAndReconnect}
              type="button"
              variant="danger"
            >
              {t("ssh.trust.resetAndReconnect")}
            </Button>
          ) : (
            <DialogClose asChild>
              <Button disabled={pending} onClick={onConfirm} type="button">
                <ShieldCheck size={14} />
                {isFirstTrust ? t("ssh.trust.trustConnect") : t("ssh.trust.connect")}
              </Button>
            </DialogClose>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
