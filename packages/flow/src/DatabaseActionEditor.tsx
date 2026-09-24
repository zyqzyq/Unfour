import { Input, useI18n } from "@unfour/ui";
import { ActionTextField } from "./ActionTextField";
import type { ActionEditorProps } from "./actionEditorTypes";
import { sqlProblem } from "./actionAuthoring";
import { ValueEditor } from "./ValueEditor";

export function DatabaseActionEditor({ action, variables, onChange, onValidity }: ActionEditorProps) {
  const { t } = useI18n();
  const set = (key: string, value: unknown) => onChange({ ...action, arguments: { ...action.arguments, [key]: value } });
  const problem = sqlProblem(action.arguments.sql);
  return <div className="grid gap-2">
    <div className="grid grid-cols-2 gap-2">{["catalog", "schema"].map((key) => <ActionTextField key={key} label={t(`flow.argument.${key}`)} value={action.arguments[key] ?? ""} variables={variables} onChange={(v) => set(key, v)} onValidity={(valid) => onValidity(key, valid)} />)}</div>
    <ActionTextField label={t("flow.argument.sql")} value={action.arguments.sql ?? ""} multiline variables={variables} onChange={(v) => set("sql", v)} onValidity={(valid) => onValidity("sql", valid)} />
    {problem && <p role="alert">{t(problem)}</p>}
    {action.arguments.limit !== undefined && action.arguments.limit !== null && typeof action.arguments.limit !== "number" ? <details><summary>{t("flow.argument.limit")} · {t("flow.advanced")}</summary><ValueEditor label={t("flow.argument.limit")} value={action.arguments.limit} variables={variables} onChange={(v) => set("limit", v)} onValidity={(valid) => onValidity("limit", valid)} /></details> : <label>{t("flow.argument.limit")}<Input aria-label={t("flow.argument.limit")} type="number" min={1} max={1000} value={action.arguments.limit === null ? "" : Number(action.arguments.limit ?? 100)} onChange={(e) => set("limit", e.target.value === "" ? null : Number(e.target.value))} /></label>}
  </div>;
}
