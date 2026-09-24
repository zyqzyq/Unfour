import { useRef, useState } from "react";
import { Button, Input, Select, useI18n } from "@unfour/ui";
import { ValueEditor } from "./ValueEditor";
import { variableReference, type Variable } from "./variables";

export function ActionTextField({ label, value, variables, onChange, onValidity, multiline = false, placeholder, compact = false }: {
  label: string; value: unknown; variables: Variable[]; onChange: (value: unknown) => void;
  onValidity: (valid: boolean) => void; multiline?: boolean; placeholder?: string; compact?: boolean;
}) {
  const { t } = useI18n();
  const selection = useRef({ start: 0, end: 0 });
  const [source, setSource] = useState("");
  const [path, setPath] = useState("");
  if (typeof value !== "string") return <details><summary>{label} · {t("flow.advanced")}</summary><ValueEditor {...{ label, value, variables, onChange, onValidity }} /></details>;
  const props = { "aria-label": label, value, placeholder, onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => onChange(e.target.value), onSelect: (e: React.SyntheticEvent<HTMLInputElement | HTMLTextAreaElement>) => { selection.current = { start: e.currentTarget.selectionStart ?? value.length, end: e.currentTarget.selectionEnd ?? value.length }; } };
  return <div className="grid min-w-0 gap-1"><span className={compact ? "sr-only" : "text-xs"}>{label}</span>
    {multiline ? <textarea {...props} rows={6} className="w-full rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono text-xs" /> : <Input {...props} />}
    <details><summary className="text-xs">{t("flow.insertVariable")}</summary><div className="flex gap-1">
      <Select aria-label={`${label} · ${t("flow.variable")}`} value={source} options={[{ value: "", label: t("flow.chooseVariable") }, ...variables.map((v) => ({ value: variableReference(v.path).$ref, label: v.label }))]} onChange={(e) => setSource(e.target.value)} />
      <Input aria-label={`${label} · ${t("flow.variablePath")}`} value={path} onChange={(e) => setPath(e.target.value)} />
      <Button size="sm" variant="ghost" disabled={!source} onClick={() => {
        const suffix = path ? variableReference(path.split(".")).$ref : "";
        const text = "${" + source + suffix + "}";
        onChange(value.slice(0, selection.current.start) + text + value.slice(selection.current.end));
        selection.current = { start: selection.current.start + text.length, end: selection.current.start + text.length };
      }}>{t("flow.insertVariable")}</Button>
    </div></details>
  </div>;
}
