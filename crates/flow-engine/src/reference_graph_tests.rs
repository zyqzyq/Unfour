use super::*;
use serde_json::json;

#[test]
fn malformed_references_are_invalid_and_escaped_multiple_references_are_valid() {
    for value in [
        json!({"$ref": null}),
        json!({"$ref": 1}),
        json!({"$ref": "/inputs/a", "extra": true}),
        json!({"$ref": "inputs/a"}),
        json!({"$ref": "/unknown/a"}),
        json!({"$ref": "/inputs/a~2b"}),
        json!({"$ref": "/inputs/a~"}),
        json!("${}"),
        json!("${inputs/a}"),
        json!("${/inputs/a"),
        json!("${/inputs/a} ${/unknown/b}"),
    ] {
        assert!(
            check(&value, &ids(&[]), false)
                .unwrap_err()
                .to_string()
                .contains("FLOW_INVALID_REFERENCE"),
            "{value}"
        );
    }
    let value = json!("${/inputs/a~1b} ${/inputs/c~0d} ${/inputs}");
    check(&value, &ids(&[]), false).unwrap();
    assert_eq!(
        crate::expression::resolve(&value, &json!({"inputs":{"a/b":"A","c~d":"B"}})).unwrap(),
        json!("A B {\"a/b\":\"A\",\"c~d\":\"B\"}")
    );
}

fn wait(id: &str, next: Option<&str>) -> Value {
    json!({"id":id,"name":id,"timeoutMs":1000,"kind":"wait","durationMs":0,"next":next})
}
fn branch(id: &str, yes: &str, no: &str) -> Value {
    json!({"id":id,"name":id,"timeoutMs":1000,"kind":"condition","predicate":{"left":true,"op":"eq","right":true},"ifTrue":yes,"ifFalse":no})
}
fn steps(values: Vec<Value>) -> Vec<FlowStep> {
    serde_json::from_value(json!(values)).unwrap()
}
fn ids(values: &[&str]) -> HashSet<String> {
    values.iter().map(|v| v.to_string()).collect()
}

#[test]
fn branches_merge_by_intersection_and_ignore_unreachable_predecessors() {
    let graph = steps(vec![
        wait("start", None),
        branch("split", "a", "b"),
        wait("a", Some("a2")),
        wait("a2", Some("join")),
        wait("b", Some("join")),
        wait("dead", None),
        wait("join", None),
        wait("tail", None),
    ]);
    let safe = guaranteed_upstream(&graph);
    assert_eq!(safe[3], ids(&["start", "split", "a"]));
    assert_eq!(safe[4], ids(&["start", "split"]));
    assert_eq!(safe[5], ids(&[]));
    assert_eq!(safe[6], ids(&["start", "split"]));
    assert_eq!(safe[7], ids(&["start", "split", "join"]));
}

#[test]
fn nested_branches_end_and_identical_targets_follow_engine_edges() {
    let graph = steps(vec![
        branch("outer", "inner", "$end"),
        branch("inner", "a", "join"),
        wait("a", Some("join")),
        branch("join", "tail", "tail"),
        wait("tail", None),
    ]);
    let safe = guaranteed_upstream(&graph);
    assert_eq!(safe[3], ids(&["outer", "inner"]));
    assert_eq!(safe[4], ids(&["outer", "inner", "join"]));
    let jump = steps(vec![
        wait("start", Some("last")),
        wait("skipped", None),
        wait("last", None),
    ]);
    assert_eq!(guaranteed_upstream(&jump)[2], ids(&["start"]));
}

#[test]
fn rejects_branch_self_future_missing_and_out_of_scope_probe_references() {
    for pointer in [
        "/steps/a/body",
        "/steps/b/body",
        "/steps/use/body",
        "/steps/future/body",
        "/steps/missing",
        "/probe/body",
    ] {
        for value in [
            json!({"nested":[{"$ref":pointer}]}),
            json!({"text":format!("prefix ${{{pointer}}} suffix")}),
        ] {
            let action = json!({"id":"use","name":"Use","kind":"action","timeoutMs":1000,"action":{"capability":"api","resourceId":"r","arguments":value}});
            let graph = steps(vec![
                branch("split", "a", "b"),
                wait("a", Some("use")),
                wait("b", None),
                action,
                wait("future", None),
            ]);
            assert!(validate_references(&graph)
                .unwrap_err()
                .to_string()
                .contains("FLOW_UNSAFE_REFERENCE"));
        }
    }
    check(
        &json!({"$ref":"/steps/split/matched"}),
        &ids(&["split"]),
        false,
    )
    .unwrap();
    check(&json!("${/steps/a~1b~0c/body}"), &ids(&["a/b~c"]), false).unwrap();
}

#[test]
fn only_predicates_can_read_current_probe_for_both_wait_shapes() {
    for kind in ["poll", "waitUntil"] {
        let mut node = json!({"id":"poll","name":"Poll","kind":kind,"timeoutMs":1000,"intervalMs":10,"maxAttempts":3,"probe":{"capability":"api","resourceId":"r","arguments":{}},"predicate":{"left":{"$ref":"/probe/body/ready"},"op":"eq","right":true},"successWhen":{"left":{"$ref":"/probe/body/ready"},"op":"eq","right":true}});
        validate_references(&steps(vec![node.clone()])).unwrap();
        node["probe"]["arguments"] = json!({"url":"${/probe/body/url}"});
        assert!(validate_references(&steps(vec![node])).is_err());
    }
}

#[test]
fn builder_encodings_keep_predicate_semantics_including_missing_errors() {
    let context = json!({"probe":{"body":{"status":"ready","ready":true}},"inputs":{"allowed":["ready","done"]}});
    for predicate in [
        json!({"left":{"$ref":"/probe/body/status"},"op":"eq","right":"ready"}),
        json!({"left":{"$ref":"/probe/body/ready"},"op":"eq","right":true}),
        json!({"left":{"$ref":"/probe/body/status"},"op":"in","right":{"$ref":"/inputs/allowed"}}),
    ] {
        assert!(crate::expression::predicate(
            &serde_json::from_value(predicate).unwrap(),
            &context
        )
        .unwrap());
    }
    let missing = serde_json::from_value(
        json!({"left":{"$ref":"/probe/body/missing"},"op":"ne","right":null}),
    )
    .unwrap();
    assert!(crate::expression::predicate(&missing, &context)
        .unwrap_err()
        .to_string()
        .contains("FLOW_MISSING_REFERENCE"));
}
