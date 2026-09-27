import { useRef, useState } from "react";
import { Button, Input, useI18n } from "@unfour/ui";
import { ValueEditor } from "./ValueEditor";
import { variableReference, type Variable } from "./variables";

import { VariablePicker } from "./VariablePicker";

export function ActionTextField({ label, value, variables, onChange, onValidity, multiline = false, placeholder, compact = false }: {
  label: string; value: unknown; variables: Variable[]; onChange: (value: unknown) => void;
  onValidity: (valid: boolean) => void; multiline?: boolean; placeholder?: string; compact?: boolean;
}) {
  const { t } = useI18n();
  const selection = useRef({ start: 0, end: 0 });
  const [source, setSource] = useState("");
  const sourceAvailable = variables.some((item) => {
    const pointer = variableReference(item.path).$ref;
    return source === pointer || (item.path[0] !== "environment" && source.startsWith(pointer + "/"));
  });
  if (typeof value !== "string") return <details><summary>{label} · {t("flow.advanced")}</summary><ValueEditor {...{ label, value, variables, onChange, onValidity }} /></details>;
  const props = { "aria-label": label, value, placeholder, onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => onChange(e.target.value), onSelect: (e: React.SyntheticEvent<HTMLInputElement | HTMLTextAreaElement>) => { selection.current = { start: e.currentTarget.selectionStart ?? value.length, end: e.currentTarget.selectionEnd ?? value.length }; } };
  return <div className="grid min-w-0 gap-1"><span className={compact ? "sr-only" : "text-xs"}>{label}</span>
    {multiline ? <textarea {...props} rows={6} className="w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono text-xs" /> : <Input {...props} />}
    <details><summary className="text-xs">{t("flow.insertVariable")}</summary><div className="flex gap-1">
      <VariablePicker qualifiedPathLabel label={label} value={source} variables={variables} onChange={setSource} />
      <Button size="sm" variant="ghost" disabled={!sourceAvailable} onClick={() => {
        const variable = variables.find((item) => item.path[0] === "environment" && variableReference(item.path).$ref === source);
        const text = variable ? "{{" + variable.path[1] + "}}" : "${" + source + "}";
        onChange(value.slice(0, selection.current.start) + text + value.slice(selection.current.end));
        selection.current = { start: selection.current.start + text.length, end: selection.current.start + text.length };
      }}>{t("flow.insertVariable")}</Button>
    </div></details>
  </div>;
}
