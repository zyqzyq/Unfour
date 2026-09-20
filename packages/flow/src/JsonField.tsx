import { useEffect, useRef, useState } from "react";
import { Input, useI18n } from "@unfour/ui";

export function JsonField({
  label, value, onChange, onValidity, optional = false, secret = false,
}: {
  label: string;
  value: unknown;
  onChange: (value: unknown) => void | boolean;
  onValidity?: (valid: boolean) => void;
  optional?: boolean;
  secret?: boolean;
}) {
  const { t } = useI18n();
  const serialized = JSON.stringify(value, null, 2) ?? "";
  const [text, setText] = useState(serialized);
  const [external, setExternal] = useState(serialized);
  const [error, setError] = useState(false);
  const validity = useRef(onValidity);
  useEffect(() => { validity.current = onValidity; }, [onValidity]);
  useEffect(() => {
    // Compare values rather than object identity so unrelated parent renders
    // never discard an incomplete JSON draft.
    if (serialized !== external) {
      setExternal(serialized);
      setText(serialized);
      setError(false);
      validity.current?.(true);
    }
  }, [serialized, external]);

  function edit(next: string) {
    setText(next);
    try {
      const parsed: unknown = optional && !next.trim() ? undefined : JSON.parse(next);
      const valid = onChange(parsed) !== false;
      if (valid) setExternal(JSON.stringify(parsed, null, 2) ?? "");
      setError(!valid);
      onValidity?.(valid);
    } catch {
      setError(true);
      onValidity?.(false);
    }
  }
  const inputProps = {
    "aria-label": label,
    "aria-invalid": error,
    value: text,
  };
  return (
    <label className="grid gap-1 text-xs">
      {label}
      {secret ? (
        <Input {...inputProps} type="password" autoComplete="new-password"
          onChange={(event) => edit(event.target.value)} />
      ) : (
        <textarea {...inputProps}
          className="min-h-20 w-full resize-y rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono text-xs focus:outline-[var(--u-color-focus)]"
          onChange={(event) => edit(event.target.value)} />
      )}
      {error && <span role="alert">{t("flow.invalidJson")}</span>}
    </label>
  );
}
