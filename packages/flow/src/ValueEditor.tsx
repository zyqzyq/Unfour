import { useEffect, useRef, useState } from "react";
import { Input, Select, useI18n } from "@unfour/ui";
import { validReferences } from "./model";
import { JsonField } from "./JsonField";
import { type Variable } from "./variables";
import { VariablePicker } from "./VariablePicker";

export function ValueEditor({ label, value, variables, onChange, onValidity, types = ["string", "number", "boolean", "null", "json", "variable"] }: {
  label: string; value: unknown; variables: Variable[]; onChange: (value: unknown) => void; onValidity: (valid: boolean) => void; types?: string[];
}) {
  const { t } = useI18n();
  const ref = value && typeof value === "object" && "$ref" in value && typeof value.$ref === "string" ? value.$ref : null;
  const kind = ref !== null ? "variable" : value === null ? "null" : typeof value === "object" ? "json" : typeof value;
  const [picking, setPicking] = useState(false);
  const validity = useRef(onValidity);
  useEffect(() => { validity.current = onValidity; }, [onValidity]);
  useEffect(() => { validity.current(validReferences(value)); }, [value]);
  // Hidden inspectors stay mounted. Only removed/replaced fields clear their draft errors.
  useEffect(() => () => validity.current(true), []);
  return <div className="grid gap-1">
    <div className="grid grid-cols-2 items-center gap-2"><span className="text-xs">{label}</span>
      <Select aria-label={`${label} · ${t("flow.inputType")}`} value={picking ? "variable" : kind} options={[...new Set([...types, kind])].map((type) => ({ value: type, label: t(`flow.value.${type}`) }))} onChange={(event) => {
        const type = event.target.value;
        setPicking(type === "variable");
        if (type === "variable") onValidity(Boolean(ref));
        if (type !== "variable") { onChange(type === "string" ? "" : type === "number" ? 0 : type === "boolean" ? false : type === "json" ? {} : null); onValidity(true); }
      }} />
    </div>
    {(picking || ref !== null) ? <>
      <VariablePicker label={label} value={ref ?? ""} variables={variables.filter((item) => item.path[0] !== "environment")} onChange={(pointer) => { onChange({ $ref: pointer }); setPicking(false); onValidity(true); }} />
    </> : kind === "string" ? <Input aria-label={label} value={String(value)} onChange={(event) => onChange(event.target.value)} />
      : kind === "number" ? <NumberValue label={label} value={Number(value)} onChange={onChange} onValidity={onValidity} />
      : kind === "boolean" ? <Select aria-label={label} value={String(value)} options={[{ value: "true", label: t("flow.true") }, { value: "false", label: t("flow.false") }]} onChange={(event) => onChange(event.target.value === "true")} />
      : kind === "json" ? <JsonField label={label} value={value} onChange={(next) => { if (!validReferences(next)) return false; onChange(next); }} onValidity={onValidity} /> : null}
  </div>;
}

function NumberValue({ label, value, onChange, onValidity }: { label: string; value: number; onChange: (value: number) => void; onValidity: (valid: boolean) => void }) {
  const { t } = useI18n();
  const [draft, setDraft] = useState({ value, text: String(value), invalid: false });
  const invalid = draft.value === value && draft.invalid;
  return <><Input aria-label={label} aria-invalid={invalid} type="number" value={draft.value === value ? draft.text : String(value)} onChange={(event) => {
    const text = event.target.value;
    const number = Number(text);
    const valid = text !== "" && Number.isFinite(number);
    setDraft({ value: valid ? number : value, text, invalid: !valid });
    onValidity(valid);
    if (valid) onChange(number);
  }} />{invalid && <span role="alert" className="text-xs">{t("flow.inputTypeError")}</span>}</>;
}
