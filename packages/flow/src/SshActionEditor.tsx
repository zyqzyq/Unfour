import { Button, Select, useI18n } from "@unfour/ui";
import { ActionTextField } from "./ActionTextField";
import type { ActionEditorProps } from "./actionEditorTypes";

export function SshActionEditor({ action, resources, variables, onChange, onValidity }: ActionEditorProps) {
  const { t } = useI18n();
  const detail = resources.ssh.find((task) => task.id === action.resourceId)?.detail;
  const inputs = action.arguments.inputs;
  const object = inputs && typeof inputs === "object" && !Array.isArray(inputs) && !("$ref" in inputs) ? inputs as Record<string, unknown> : null;
  return <div className="grid gap-2">
    <Select aria-label={t("flow.connection")} value={action.connectionId ?? ""} options={[{ value: "", label: t("flow.selectConnection") }, ...resources.connections.map((c) => ({ value: c.id, label: c.name }))]} onChange={(e) => onChange({ ...action, connectionId: e.target.value || null })} />
    <p className="text-xs">{t("flow.sshDefaultsHelp")}</p>
    {detail && object ? (detail.detectedInputs ?? []).map((name) => <div key={name} className="grid gap-1">
      <ActionTextField label={name} value={object[name] ?? ""} placeholder={action.arguments.workspaceDefaults === true && !(name in object) ? t("flow.workspaceDefault") : undefined} variables={variables} onChange={(value) => onChange({ ...action, arguments: { ...action.arguments, inputs: { ...object, [name]: value } } })} onValidity={(valid) => onValidity(name, valid)} />
      {action.arguments.workspaceDefaults === true && name in object && <Button size="sm" variant="ghost" onClick={() => { const next = { ...object }; delete next[name]; onChange({ ...action, arguments: { ...action.arguments, inputs: next } }); }}>{t("flow.workspaceDefault")}</Button>}
    </div>) : <p>{t(detail ? "flow.editAdvanced" : "flow.loading")}</p>}
    {action.arguments.workspaceDefaults !== true && <Button size="sm" variant="secondary" onClick={() => onChange({ ...action, arguments: { ...action.arguments, workspaceDefaults: true } })}>{t("flow.enableWorkspaceDefaults")}</Button>}
  </div>;
}
