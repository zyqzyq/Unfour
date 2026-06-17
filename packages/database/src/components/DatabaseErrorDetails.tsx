import { confirmationMessage, describeDatabaseError } from "../result-utils";

export function DatabaseErrorDetails({
  confirmation = false,
  error,
}: {
  confirmation?: boolean;
  error: unknown;
}) {
  const description = describeDatabaseError(error);
  const message = confirmation ? confirmationMessage(error) : description.message;

  return (
    <div className="min-w-0 space-y-1 text-left">
      <div className="font-semibold">{description.title}</div>
      <div className="break-words">{message}</div>
      {description.technicalDetail ? (
        <details className="mt-1">
          <summary className="cursor-pointer text-[11px]">Technical detail</summary>
          <pre className="mt-1 max-h-28 overflow-auto whitespace-pre-wrap break-words rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono text-[11px] text-[var(--u-color-text-muted)]">
            {description.technicalDetail}
          </pre>
        </details>
      ) : null}
    </div>
  );
}
