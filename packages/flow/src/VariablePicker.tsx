import { useState } from "react";
import { Input, Select, useI18n } from "@unfour/ui";
import { variableReference, type Variable } from "./variables";

export function VariablePicker({ label, value, variables, onChange, qualifiedPathLabel = false }: {
  qualifiedPathLabel?: boolean; label: string; value: string; variables: Variable[]; onChange: (pointer: string) => void;
}) {
  const { t } = useI18n();
  const [chosenSource, setChosenSource] = useState("");
  const chosen = variables.find((item) => variableReference(item.path).$ref === chosenSource);
  const candidates = [...variables].sort((a, b) => b.path.length - a.path.length);
  const source = [...(chosen ? [chosen] : []), ...candidates].find((item) => {
    const pointer = variableReference(item.path).$ref;
    return value === pointer || value.startsWith(pointer + "/");
  });
  const sourceRef = source ? variableReference(source.path).$ref : "";
  const path = source && value !== sourceRef ? value.slice(sourceRef.length + 1).split("/").map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~")).join(".") : "";
  const groups = ["inputs", "environment", "steps", "probe"];
  return <div className="grid gap-1">
    {value && !source && <p className="break-all text-xs text-[var(--u-color-danger)]">{t("flow.unavailableVariable")} · {value.split("/").slice(1).map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~")).join(" · ")}</p>}
    <Select aria-label={`${label} · ${t("flow.variable")}`} value={sourceRef} onChange={(event) => {
      if (event.target.value) { setChosenSource(event.target.value); onChange(event.target.value); }
    }}>
      <option value="">{t("flow.chooseVariable")}</option>
      {groups.map((group) => <optgroup key={group} label={t(`flow.variableGroups.${group}`)}>
        {variables.filter((item) => item.path[0] === group).map((item) => <option key={variableReference(item.path).$ref} value={variableReference(item.path).$ref}>{item.label}</option>)}
      </optgroup>)}
    </Select>
    <label className="grid gap-1 text-xs">{t("flow.variablePath")}<Input aria-label={qualifiedPathLabel ? `${label} · ${t("flow.variablePath")}` : undefined} value={path} disabled={!source || source.path[0] === "environment"} onChange={(event) => {
      if (source) onChange(variableReference([...source.path, ...(event.target.value ? event.target.value.split(".") : [])]).$ref);
    }} /></label>
    <details className="text-xs text-[var(--u-color-text-muted)]"><summary>{t("flow.variablePathHelp")}</summary><p>{t("flow.variableHelp")}</p></details>
  </div>;
}
