import { useEffect, useRef, useState } from "react";
import { Input, Select, useI18n } from "@unfour/ui";
import { JsonField } from "./JsonField";
import { variableReference, type Variable } from "./variables";

export function ValueEditor({ label, value, variables, onChange, onValidity, types = ["string", "number", "boolean", "null", "json", "variable"] }: {
  label: string; value: unknown; variables: Variable[]; onChange: (value: unknown) => void; onValidity: (valid: boolean) => void; types?: string[];
}) {
  const { t } = useI18n();
  const ref = value && typeof value === "object" && "$ref" in value && typeof value.$ref === "string" ? value.$ref : null;
  const kind = ref !== null ? "variable" : value === null ? "null" : typeof value === "object" ? "json" : typeof value;
  const [picking, setPicking] = useState(false);
  const [chosenSource, setChosenSource] = useState("");
  const candidates = [...variables].sort((a, b) => b.path.length - a.path.length);
  const chosen = variables.find((item) => variableReference(item.path).$ref === chosenSource);
  const source = ref ? [...(chosen ? [chosen] : []), ...candidates].find((item) => {
    const pointer = variableReference(item.path).$ref;
    return ref === pointer || ref.startsWith(pointer + "/");
  }) : undefined;
  const sourceRef = source ? variableReference(source.path).$ref : "";
  const path = source && ref ? ref.slice(sourceRef.length + 1).split("/").map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~")).join(".") : "";
  const validity = useRef(onValidity);
  useEffect(() => { validity.current = onValidity; }, [onValidity]);
  useEffect(() => { validity.current(true); }, [value]);
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
      {ref && <span className="break-all text-xs">{variables.find((item) => variableReference(item.path).$ref === ref)?.label ?? ref.split("/").slice(1).map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~")).join(" · ")}</span>}
      <Select aria-label={`${label} · ${t("flow.variable")}`} value={sourceRef} options={[{ value: "", label: t("flow.chooseVariable") }, ...variables.map((item) => ({ value: variableReference(item.path).$ref, label: `${t(item.path[0] === "inputs" ? "flow.canvas.start" : item.path[0] === "probe" ? "flow.probeResult" : "flow.outputs")} · ${item.label}` }))]} onChange={(event) => {
        const variable = variables.find((item) => variableReference(item.path).$ref === event.target.value);
        if (variable) { setChosenSource(event.target.value); onChange(variableReference(variable.path)); setPicking(false); onValidity(true); }
      }} />
      <label className="grid gap-1 text-xs">{t("flow.variablePath")}<Input value={path} disabled={!source} onChange={(event) => {
        if (source) { onChange(variableReference([...source.path, ...(event.target.value ? event.target.value.split(".") : [])])); onValidity(true); }
      }} /></label>
      <p className="text-xs text-[var(--u-color-text-muted)]">{t("flow.variableHelp")}</p>
    </> : kind === "string" ? <Input aria-label={label} value={String(value)} onChange={(event) => onChange(event.target.value)} />
      : kind === "number" ? <NumberValue label={label} value={Number(value)} onChange={onChange} onValidity={onValidity} />
      : kind === "boolean" ? <Select aria-label={label} value={String(value)} options={[{ value: "true", label: t("flow.true") }, { value: "false", label: t("flow.false") }]} onChange={(event) => onChange(event.target.value === "true")} />
      : kind === "json" ? <JsonField label={label} value={value} onChange={onChange} onValidity={onValidity} /> : null}
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
