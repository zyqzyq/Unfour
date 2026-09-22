use serde_json::{json, Value};

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object", "properties":properties, "required":required, "additionalProperties":false})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn string() -> Value {
    json!({"type":"string"})
}
fn nullable_string() -> Value {
    json!({"type":["string","null"]})
}
fn reference(name: &str) -> Value {
    json!({"$ref":format!("#/$defs/{name}")})
}
fn definitions() -> Value {
    let predicate = object(
        json!({"left":{}, "op":{"enum":["eq","ne","gt","ge","lt","le","in"]}, "right":{}}),
        &["left", "op", "right"],
    );
    let action = object(
        json!({"capability":{"enum":["api","ssh","database"]},"resourceId":string(),"connectionId":nullable_string(),"arguments":{"type":"object"}}),
        &["capability", "resourceId", "arguments"],
    );
    let input = object(
        json!({"name":string(),"type":{"enum":["string","number","boolean","json"]},"required":{"type":"boolean"},"default":{},"secret":{"type":"boolean"},"description":nullable_string()}),
        &["name", "type"],
    );
    let mut variants = vec![];
    for (kind, fields, required) in [
        (
            "action",
            json!({"action":reference("action")}),
            vec!["action"],
        ),
        (
            "condition",
            json!({"predicate":reference("predicate"),"ifTrue":string(),"ifFalse":string()}),
            vec!["predicate", "ifTrue", "ifFalse"],
        ),
        (
            "poll",
            json!({"probe":reference("action"),"predicate":reference("predicate"),"intervalMs":{"type":"integer","minimum":0},"maxAttempts":{"type":"integer","minimum":0}}),
            vec!["probe", "predicate", "intervalMs", "maxAttempts"],
        ),
        (
            "waitUntil",
            json!({"probe":reference("action"),"successWhen":reference("predicate"),"failureWhen":{"anyOf":[reference("predicate"),{"type":"null"}]},"intervalMs":{"type":"integer","minimum":0},"maxAttempts":{"type":["integer","null"],"minimum":0},"probeErrorPolicy":{"enum":["failImmediately","retryTransientErrors"]},"intervalStrategy":{"enum":["fixed"]}}),
            vec!["probe", "successWhen", "intervalMs"],
        ),
        (
            "wait",
            json!({"durationMs":{"type":"integer","minimum":0}}),
            vec!["durationMs"],
        ),
    ] {
        let mut props = json!({"id":string(),"name":string(),"timeoutMs":{"type":"integer","minimum":0},"next":nullable_string(),"kind":{"const":kind}});
        props
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let mut keys = vec!["id", "name", "timeoutMs", "kind"];
        keys.extend(required);
        variants.push(object(props, &keys));
    }
    let definition = object(
        json!({"id":string(),"workspaceId":string(),"name":string(),"revision":{"type":"integer"},"inputs":array(json!({"anyOf":[string(), reference("input")]})),"steps":array(json!({"oneOf":variants}))}),
        &["id", "workspaceId", "name", "revision", "inputs", "steps"],
    );
    let status = json!({"enum":["running","succeeded","failed","timedOut","cancelled","interrupted","validationFailed"]});
    let summary = object(
        json!({"id":string(),"flowId":string(),"status":status,"startedAt":string(),"finishedAt":nullable_string()}),
        &["id", "flowId", "status", "startedAt", "finishedAt"],
    );
    let context = object(
        json!({"workspaceId":string(),"flowId":string(),"environmentId":nullable_string(),"inputs":{"type":"object"},"secretInputNames":array(string()),"initiator":{"enum":["human","mcp"]},"confirmEffects":{"type":"boolean"}}),
        &[
            "workspaceId",
            "flowId",
            "environmentId",
            "inputs",
            "secretInputNames",
            "initiator",
            "confirmEffects",
        ],
    );
    let attempt = object(
        json!({"number":{"type":"integer"},"input":{},"output":{},"error":nullable_string(),"durationMs":{"type":"integer"}}),
        &["number", "input", "output", "error", "durationMs"],
    );
    let step_run = object(
        json!({"stepId":string(),"status":{"enum":["pending","running","succeeded","failed","timedOut","cancelled","interrupted","skipped"]},"durationMs":{"type":"integer"},"attempts":array(attempt),"error":nullable_string(),"startedAt":nullable_string(),"nextCheckAt":nullable_string(),"output":{}}),
        &[
            "stepId",
            "status",
            "durationMs",
            "attempts",
            "error",
            "startedAt",
            "nextCheckAt",
            "output",
        ],
    );
    let run = object(
        json!({"id":string(),"workspaceId":string(),"flowId":string(),"definition":reference("definition"),"context":context,"resources":{},"status":status,"error":nullable_string(),"startedAt":string(),"finishedAt":nullable_string(),"steps":array(step_run)}),
        &[
            "id",
            "workspaceId",
            "flowId",
            "definition",
            "context",
            "resources",
            "status",
            "error",
            "startedAt",
            "finishedAt",
            "steps",
        ],
    );
    json!({"predicate":predicate,"action":action,"input":input,"definition":definition,"summary":summary,"run":run})
}
pub(super) fn input(name: &str) -> Value {
    let mut properties =
        json!({"workspaceId":{"type":"string","minLength":1,"pattern":"^\\S(?:.*\\S)?$"}});
    let mut required = vec![];
    match name {
        "unfour.flow.get" | "unfour.flow.list_runs" | "unfour.flow.run" => {
            properties["flowId"] =
                json!({"type":"string","minLength":1,"pattern":"^\\S(?:.*\\S)?$"});
            required.push("flowId");
        }
        "unfour.flow.get_run" | "unfour.flow.cancel_run" => {
            properties["runId"] =
                json!({"type":"string","minLength":1,"pattern":"^\\S(?:.*\\S)?$"});
            required.push("runId");
        }
        "unfour.flow.save" => {
            properties["definition"] = reference("definition");
            required.push("definition");
        }
        _ => {}
    }
    if name == "unfour.flow.run" {
        properties.as_object_mut().unwrap().extend(json!({"environmentId":nullable_string(),"inputs":{"type":"object"},"secretInputNames":array(string()),"confirm":{"type":"boolean"},"confirmationText":string(),"confirmation_text":string()}).as_object().unwrap().clone());
    }
    let mut schema = object(properties, &required);
    schema["$defs"] = definitions();
    schema
}
pub(super) fn output(name: &str) -> Value {
    let (key, value) = match name {
        "unfour.flow.list" => ("flows", array(reference("definition"))),
        "unfour.flow.get" | "unfour.flow.save" => ("flow", reference("definition")),
        "unfour.flow.list_runs" => ("runs", array(reference("summary"))),
        _ => ("run", reference("run")),
    };
    let mut schema = object(json!({key:value}), &[key]);
    schema["$defs"] = definitions();
    schema
}
