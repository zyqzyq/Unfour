import { Button, ErrorState, useI18n } from "@unfour/ui";
import { useAccount } from "./useAccount";

export function AccountFailureNotice() {
  const { t } = useI18n();
  const { failure } = useAccount();
  if (!failure) return null;
  const suggestion = failure.stage === "credentialSave" ? "credentialSave"
    : failure.stage === "codeExchange" && failure.category === "temporary" ? "exchangeUncertain"
      : failure.code === "installation_id_unavailable" ? "prepareSignIn"
      : failure.code === "browser_open_failed" ? "browser"
        : failure.stage === "credentialRead" && failure.recovery === "refreshAccount" ? "credentialRead"
          : failure.recovery;
  return (
    <ErrorState>
      <div className="flex min-w-0 flex-col gap-1 text-left" role="alert">
        <p>{t(`account.failureCategory.${failure.category}`)}</p>
        <p className="break-words text-xs">{t("account.failureDetails", {
          code: failure.code, stage: t(`account.failureStage.${failure.stage}`),
        })}</p>
        <p className="text-xs">{t(`account.recoveryHint.${suggestion}`)}</p>
      </div>
    </ErrorState>
  );
}

export function AccountRecoveryButton() {
  const { t } = useI18n();
  const { failure, state, retry, refreshing } = useAccount();
  if (!failure && state.kind !== "error" && state.kind !== "signingIn") return null;
  const recovery = failure?.recovery ?? (state.kind === "signingIn" ? "restartSignIn" : "refreshAccount");
  if (recovery === "restartApp") return null;
  return <Button disabled={refreshing} onClick={retry} size="sm" type="button">
    {t(`account.recoveryAction.${recovery}`)}
  </Button>;
}
