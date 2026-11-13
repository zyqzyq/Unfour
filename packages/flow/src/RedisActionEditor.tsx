import { useI18n } from "@unfour/ui";
import { ActionTextField } from "./ActionTextField";
import type { ActionEditorProps } from "./actionEditorTypes";

export function RedisActionEditor({ action, variables, onChange, onValidity }: ActionEditorProps) {
  const { t } = useI18n();
  function input(name: "key" | "field" | "equals") {
    return <ActionTextField label={t(`flow.redisArgs.${name}`)}
      value={action.arguments[name] ?? ""} variables={variables}
      onValidity={(valid) => onValidity(name, valid)}
      onChange={(value) => onChange({ ...action, arguments: { ...action.arguments, [name]: value } })} />;
  }
  return <div className="grid gap-2">
    {input("key")}
    {(["field", "equals"] as const).map((name) => {
      const enabled = name in action.arguments;
      return <div className="grid gap-1" key={name}>
        <label className="flex items-center gap-2 text-xs">
          <input type="checkbox" checked={enabled} onChange={(event) => {
            const args = { ...action.arguments };
            if (event.target.checked) args[name] = "";
            else delete args[name];
            onValidity(name, true);
            onChange({ ...action, arguments: args });
          }} />
          {t(name === "field" ? "flow.redisArgs.checkField" : "flow.redisArgs.compareValue")}
        </label>
        {enabled && input(name)}
      </div>;
    })}
  </div>;
}
