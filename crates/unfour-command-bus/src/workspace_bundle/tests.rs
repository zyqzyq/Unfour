use super::*;
use crate::{CommandBusExtensions, TransactionalCommandHook};
use std::{future::Future, pin::Pin, sync::Arc};

pub(super) fn fixture() -> Value {
    json!({"format":"unfour-workspace","version":1,
    "workspace":{"id":"ws","name":"Portable","environmentType":"dev"},
    "variables":[{"id":"var","key":"endpoint","value":{"kind":"plain","value":"https://example.test"},"isSecret":false,"isEnabled":true,"description":null,"sortOrder":0},
    {"id":"secret","key":"access_token","value":{"kind":"plain","value":"NEVER_EXPORT_VARIABLE"},"isSecret":true,"isEnabled":true,"description":null,"sortOrder":1}],
    "environments":[{"id":"env","name":"Development","sortOrder":0}],
    "environmentVariables":[{"id":"ev","environmentId":"env","key":"region","value":{"kind":"plain","value":"local"},"isSecret":false,"isEnabled":true,"description":null,"sortOrder":0}],
    "collections":[{"id":"collection","name":"API","description":null}],
    "folders":[{"id":"child","collectionId":"collection","parentFolderId":"folder","name":"Child","sortOrder":0},{"id":"folder","collectionId":"collection","parentFolderId":null,"name":"Root","sortOrder":0}],
    "requests":[{"id":"request","collectionId":"collection","parentFolderId":"child","name":"Get","sortOrder":0,"authJson":"{\"type\":\"bearer\",\"token\":\"NEVER_EXPORT_AUTH\"}","method":"GET","url":"https://example.test?token=NEVER_EXPORT_URL","headers":[{"key":"Authorization","value":"NEVER_EXPORT_HEADER","enabled":true}],"query":[],"body":null,"bodyKind":"none","settingsJson":"{\"timeoutMs\":null}","preRequestScript":null,"postResponseScript":null,"scriptSchemaVersion":1}],
    "connections":[{"id":"ssh","connectionType":"ssh","name":"Host","host":"example.test","port":22,"config":{"kind":"ssh","username":"user","authMethod":"password"}},
    {"id":"db","connectionType":"database","name":"DB","host":"localhost","port":5432,"config":{"kind":"database","driver":"postgres","databaseName":"example","username":"user","sslMode":null,"readOnly":true}},
    {"id":"sqlite","connectionType":"database","name":"File DB","host":null,"port":null,"config":{"kind":"database","driver":"sqlite","databaseName":null,"username":null,"sslMode":null,"readOnly":true}}],
    "sshTasks":[{"id":"task","name":"Status","description":"","sortOrder":0}],
    "sshSteps":[{"id":"taskstep","taskId":"task","name":"Status","stepType":"command","position":0,"enabled":true,"configVersion":1,"configJson":{"command":"echo ready","workingDirectory":"","timeoutSeconds":30,"continueOnError":false}},
    {"id":"upload","taskId":"task","name":"Upload","stepType":"upload","position":1,"enabled":true,"configVersion":1,"configJson":{"localPath":"C:/NEVER_EXPORT_PATH/file","remotePath":"/tmp/file","overwrite":false}}],
    "savedSql":[{"id":"sql","name":"Select","sql":"select 1","connectionId":"db","catalog":"example","schema":"public"}],
    "flows":[{"id":"flow","name":"Check services","inputs":[],"steps":[
    {"id":"api","name":"API","timeoutMs":1000,"next":"remote","kind":"action","action":{"capability":"api","resourceId":"request","connectionId":null,"arguments":{}}},
    {"id":"remote","name":"SSH","timeoutMs":1000,"next":null,"kind":"action","action":{"capability":"ssh","resourceId":"task","connectionId":"ssh","arguments":{"inputs":{}}}},
    {"id":"query","name":"SQL","timeoutMs":1000,"next":null,"kind":"action","action":{"capability":"database","resourceId":"db","connectionId":null,"arguments":{"sql":"select 1"}}},
    {"id":"condition","name":"Condition","timeoutMs":1000,"next":null,"kind":"condition","predicate":{"left":{"$ref":"/steps/api/status"},"op":"eq","right":200},"ifTrue":"wait","ifFalse":"$end"},
    {"id":"wait","name":"Wait","timeoutMs":1000,"next":null,"kind":"waitUntil","probe":{"capability":"api","resourceId":"request","connectionId":null,"arguments":{"url":"https://example.test/${/steps/api/status}"}},"successWhen":{"left":{"$ref":"/probe/status"},"op":"eq","right":200},"failureWhen":null,"intervalMs":10,"maxAttempts":2}
    ]}]})
}
async fn bus() -> CommandBus {
    CommandBus::ephemeral().await.unwrap()
}
async fn count(bus: &CommandBus, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(bus.db.pool())
        .await
        .unwrap()
}
#[tokio::test]
async fn bundle_export_file_name_uses_the_exported_workspace_name() {
    let bus = bus().await;
    let selected = bus
        .create_workspace("客户 / API: v2?".into())
        .await
        .unwrap();
    bus.create_workspace("Other Workspace".into())
        .await
        .unwrap();
    let artifact = bus
        .workspace_bundle_export_artifact(selected.id)
        .await
        .unwrap();
    let bundle: Value = serde_json::from_str(&artifact.content).unwrap();
    assert_eq!(bundle["workspace"]["name"], "客户 / API: v2?");
    assert_eq!(
        artifact.suggested_file_name,
        "客户-API-v2.unfour-workspace.json"
    );
}
#[tokio::test]
async fn bundle_preview_exposes_environment_type_without_importing() {
    let bus = bus().await;
    let before = count(&bus, "workspaces").await;
    for version in [1, 2] {
        for (environment, effective) in [
            ("dev", "dev"),
            ("test", "test"),
            ("prod", "prod"),
            (" DEV ", "dev"),
            ("TEST", "test"),
            (" Prod ", "prod"),
            ("", "dev"),
        ] {
            let mut bundle = fixture();
            bundle["version"] = json!(version);
            bundle["workspace"]["environmentType"] = json!(environment);
            let input = bundle.to_string();
            let previews = [
                bus.workspace_bundle_preview(&input).await.unwrap(),
                bus.workspace_bundle_preview_with_options(
                    &input,
                    WorkspaceBundleOptions::default(),
                )
                .await
                .unwrap(),
            ];
            for preview in previews {
                assert_eq!(preview.environment_type, effective);
                assert_eq!(
                    serde_json::to_value(preview).unwrap()["environmentType"],
                    effective
                );
            }
        }
    }
    assert_eq!(count(&bus, "workspaces").await, before);
}

#[tokio::test]
async fn bundle_remaps_every_resource_and_flow_edge_on_repeated_import() {
    let bus = bus().await;
    let input = fixture().to_string();
    let before = count(&bus, "workspaces").await;
    let preview = bus.workspace_bundle_preview(&input).await.unwrap();
    assert_eq!(preview.counts["flows"], 1);
    assert_eq!(count(&bus, "workspaces").await, before);
    assert!(preview.reconfigure.iter().any(|r| r.code == "localPath"));
    let a = bus
        .workspace_bundle_import(input.clone(), "Portable".into())
        .await
        .unwrap();
    let b = bus
        .workspace_bundle_import(input, "Portable".into())
        .await
        .unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(b.name, "Portable (Copy 1)");
    assert_eq!(a.mcp_policy, "disabled");
    let a: Value = serde_json::from_str(&bus.workspace_bundle_export(a.id).await.unwrap()).unwrap();
    let b: Value = serde_json::from_str(&bus.workspace_bundle_export(b.id).await.unwrap()).unwrap();
    for key in [
        "workspace",
        "variables",
        "environments",
        "environmentVariables",
        "collections",
        "folders",
        "requests",
        "connections",
        "sshTasks",
        "sshSteps",
        "savedSql",
        "flows",
    ] {
        if key == "workspace" {
            assert_ne!(a[key]["id"], b[key]["id"]);
        } else {
            for row in a[key].as_array().unwrap() {
                assert!(!b[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["id"] == row["id"]));
            }
        }
    }
    assert_eq!(
        a["environmentVariables"][0]["environmentId"],
        a["environments"][0]["id"]
    );
    let child = a["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Child")
        .unwrap();
    let parent = a["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Root")
        .unwrap();
    assert_eq!(child["parentFolderId"], parent["id"]);
    assert_eq!(a["requests"][0]["parentFolderId"], child["id"]);
    assert_eq!(a["requests"][0]["collectionId"], a["collections"][0]["id"]);
    let db = a["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "DB")
        .unwrap();
    let ssh = a["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "Host")
        .unwrap();
    assert_eq!(a["savedSql"][0]["connectionId"], db["id"]);
    for step in a["sshSteps"].as_array().unwrap() {
        assert_eq!(step["taskId"], a["sshTasks"][0]["id"]);
    }
    let steps = a["flows"][0]["steps"].as_array().unwrap();
    assert_eq!(steps[0]["next"], steps[1]["id"]);
    assert_eq!(steps[0]["action"]["resourceId"], a["requests"][0]["id"]);
    assert_eq!(steps[1]["action"]["resourceId"], a["sshTasks"][0]["id"]);
    assert_eq!(steps[1]["action"]["connectionId"], ssh["id"]);
    assert_eq!(steps[2]["action"]["resourceId"], db["id"]);
    assert_eq!(steps[3]["ifTrue"], steps[4]["id"]);
    assert_eq!(steps[3]["ifFalse"], "$end");
    assert_eq!(
        steps[3]["predicate"]["left"]["$ref"],
        format!("/steps/{}/status", steps[0]["id"].as_str().unwrap())
    );
    assert!(steps[4]["probe"]["arguments"]["url"]
        .as_str()
        .unwrap()
        .contains(steps[0]["id"].as_str().unwrap()));
    assert_eq!(steps[4]["successWhen"]["left"]["$ref"], "/probe/status");
}
#[tokio::test]
async fn bundle_export_has_no_secrets_paths_runtime_or_sync_state() {
    let bus = bus().await;
    let workspace = bus
        .workspace_bundle_import(fixture().to_string(), "Safe".into())
        .await
        .unwrap();
    // Seed real source-only secrets/device paths. Export must use snapshots, not raw connection models.
    sqlx::query("UPDATE workspace_variables SET value='NEVER_EXPORT_STORED' WHERE workspace_id=? AND is_secret=1").bind(&workspace.id).execute(bus.db.pool()).await.unwrap();
    sqlx::query(
        "UPDATE connections SET credential_ref='NEVER_EXPORT_CREDENTIAL' WHERE workspace_id=?",
    )
    .bind(&workspace.id)
    .execute(bus.db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE ssh_connections SET config_json = ? WHERE connection_id IN (SELECT id FROM connections WHERE workspace_id = ?)")
        .bind(r#"{"keyPath":"C:/NEVER_EXPORT_PRIVATE_KEY"}"#).bind(&workspace.id).execute(bus.db.pool()).await.unwrap();
    sqlx::query("UPDATE database_connections SET config_json = ? WHERE driver = 'sqlite' AND connection_id IN (SELECT id FROM connections WHERE workspace_id = ?)")
        .bind(r#"{"sqlitePath":"C:/NEVER_EXPORT_SQLITE"}"#).bind(&workspace.id).execute(bus.db.pool()).await.unwrap();
    sqlx::query("UPDATE workspace_local_state SET active_environment_id = (SELECT id FROM workspace_environments WHERE workspace_id = ?) WHERE workspace_id = ?")
        .bind(&workspace.id).bind(&workspace.id).execute(bus.db.pool()).await.unwrap();
    let output = bus
        .workspace_bundle_export(workspace.id.clone())
        .await
        .unwrap();
    assert!(!output.contains("NEVER_EXPORT"));
    for field in [
        "credentialRef",
        "credential_ref",
        "workspaceId",
        "revision",
        "syncStatus",
        "createdAt",
        "mcpPolicy",
        "history",
        "activeEnvironmentId",
    ] {
        assert!(!output.contains(&format!("\"{field}\"")), "{field}");
    }
    let imported = bus
        .workspace_bundle_import(output, "Safe copy".into())
        .await
        .unwrap();
    let credentials: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM connections WHERE workspace_id=? AND credential_ref IS NOT NULL",
    )
    .bind(&imported.id)
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(credentials, 0);
    let active: Option<String> = sqlx::query_scalar(
        "SELECT active_environment_id FROM workspace_local_state WHERE workspace_id = ?",
    )
    .bind(&imported.id)
    .fetch_one(bus.db.pool())
    .await
    .unwrap();
    assert_eq!(active, None);
    let paths: Vec<String> = sqlx::query_scalar("SELECT config_json FROM ssh_connections WHERE connection_id IN (SELECT id FROM connections WHERE workspace_id = ?)").bind(&imported.id).fetch_all(bus.db.pool()).await.unwrap();
    assert!(paths.iter().all(|v| !v.contains("NEVER_EXPORT")));
}
#[tokio::test]
async fn bundle_rejects_missing_wrong_kind_duplicate_cyclic_and_unsupported_references() {
    let bus = bus().await;
    let baseline = count(&bus, "workspaces").await;
    let mut variants = Vec::new();
    let mut v = fixture();
    v["version"] = json!(99);
    variants.push(v);
    let mut v = fixture();
    v["requests"][0]["id"] = json!("task");
    variants.push(v);
    let mut v = fixture();
    v["flows"][0]["steps"][0]["action"]["resourceId"] = json!("missing");
    variants.push(v);
    let mut v = fixture();
    v["flows"][0]["steps"][1]["action"]["connectionId"] = json!("db");
    variants.push(v);
    let mut v = fixture();
    v["folders"][1]["parentFolderId"] = json!("child");
    variants.push(v);
    let mut v = fixture();
    v["environmentVariables"][0]["environmentId"] = json!("missing");
    variants.push(v);
    let mut v = fixture();
    v["flows"][0]["steps"][0]["action"]["arguments"] = json!({"url":{"$ref":"/steps/wait/status"}});
    variants.push(v);
    let mut v = fixture();
    v["connections"][0]["credentialRef"] = json!("forbidden");
    variants.push(v);
    let mut v = fixture();
    v["sshSteps"][1]["configJson"] = json!("bad");
    variants.push(v);
    let mut v = fixture();
    v["connections"][0]["config"]["keyPath"] = json!("forbidden");
    variants.push(v);
    for v in variants {
        assert!(bus.workspace_bundle_preview(&v.to_string()).await.is_err());
        assert!(bus
            .workspace_bundle_import(v.to_string(), "Broken".into())
            .await
            .is_err());
    }
    assert!(bus.workspace_bundle_preview("not JSON").await.is_err());
    assert_eq!(count(&bus, "workspaces").await, baseline);
}
pub(super) struct FailCommit;
impl TransactionalCommandHook for FailCommit {
    fn on_mutations<'a>(
        &'a self,
        _: &'a mut sqlx::SqliteConnection,
        context: &'a CommandContext,
        mutations: &'a [DomainMutation],
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            assert_eq!(context.origin, MutationOrigin::Local);
            assert!(mutations.iter().all(|m| m.origin == MutationOrigin::Local));
            Err(AppError::Validation("injected commit failure".into()))
        })
    }
}
#[tokio::test]
async fn bundle_rolls_back_domain_failure_and_final_transaction_hook() {
    let mut bus = bus().await;
    let tables = [
        "workspaces",
        "api_collections",
        "api_requests",
        "connections",
        "ssh_task",
        "saved_sql",
        "flow_definitions",
    ];
    let mut baseline = Vec::new();
    for t in tables {
        baseline.push(count(&bus, t).await);
    }
    let mut broken = fixture();
    broken["savedSql"][0]["sql"] = json!("");
    assert!(bus
        .workspace_bundle_import(broken.to_string(), "Broken".into())
        .await
        .is_err());
    for (t, n) in tables.iter().zip(&baseline) {
        assert_eq!(count(&bus, t).await, *n);
    }
    bus.extensions = CommandBusExtensions::new(vec![Arc::new(FailCommit)]);
    assert!(bus
        .workspace_bundle_import(fixture().to_string(), "Broken".into())
        .await
        .is_err());
    for (t, n) in tables.iter().zip(&baseline) {
        assert_eq!(count(&bus, t).await, *n);
    }
}

#[tokio::test]
async fn bundle_multipart_parts_and_poll_references_are_portable() {
    let bus = bus().await;
    let mut value = fixture();
    let mut upload = value["requests"][0].clone();
    upload["id"] = json!("upload-request");
    upload["name"] = json!("Upload");
    upload["bodyKind"] = json!(MULTIPART_BODY_KIND);
    upload["body"] = json!(json!([
        {"id":"text-part","type":"text","key":"token","enabled":true,"value":"NEVER_EXPORT_MULTIPART"},
        {"id":"file-part","type":"file","key":"file","enabled":true,"fileName":"data.txt"}
    ]).to_string());
    value["requests"].as_array_mut().unwrap().push(upload);
    value["flows"][0]["steps"][4] = json!({"id":"wait","name":"Poll","kind":"poll","timeoutMs":1000,"next":null,
        "probe":{"capability":"database","resourceId":"db","connectionId":null,"arguments":{"sql":"select 1"}},
        "predicate":{"left":{"$ref":"/probe/rowCount"},"op":"eq","right":{"$ref":"/steps/query/rowCount"}},"intervalMs":10,"maxAttempts":2});
    let imported = bus
        .workspace_bundle_import(value.to_string(), "Multipart".into())
        .await
        .unwrap();
    let content = bus.workspace_bundle_export(imported.id).await.unwrap();
    assert!(!content.contains("NEVER_EXPORT"));
    let bundle: Value = serde_json::from_str(&content).unwrap();
    let upload = bundle["requests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == "Upload")
        .unwrap();
    let parts: Value = serde_json::from_str(upload["body"].as_str().unwrap()).unwrap();
    assert_ne!(parts[0]["id"], "text-part");
    assert_ne!(parts[1]["id"], "file-part");
    let steps = &bundle["flows"][0]["steps"];
    assert_eq!(
        steps[4]["predicate"]["right"]["$ref"],
        format!("/steps/{}/rowCount", steps[2]["id"].as_str().unwrap())
    );
    assert_eq!(steps[4]["predicate"]["left"]["$ref"], "/probe/rowCount");
}
#[tokio::test]
async fn bundle_sanitizes_untrusted_payloads_without_panics_or_secret_defaults() {
    let bus = bus().await;
    let mut value = fixture();
    value["requests"][0]["settingsJson"] =
        json!(r#"{"timeoutMs":10,"token":"NEVER_EXPORT_SETTINGS"}"#);
    // Assemble PEM markers at runtime while preserving private-key redaction coverage.
    let pem_label = "PRIVATE KEY";
    value["requests"][0]["preRequestScript"] = json!(format!(
        "const token = 'NEVER_EXPORT_SCRIPT'; // {{{{unrelated}}}}\n-----BEGIN {pem_label}-----\nNEVER_EXPORT_KEY_BYTES\n-----END {pem_label}-----"
    ));
    value["flows"][0]["inputs"] = json!([
        {"name":"credential","type":"string","secret":true,"default":"NEVER_EXPORT_DEFAULT"},
        {"name":"options","type":"json","default":{"token":"NEVER_EXPORT_JSON_DEFAULT"}}
    ]);
    let parsed = parse(&value.to_string()).unwrap();
    let output = serde_json::to_string(&parsed).unwrap();
    assert!(!output.contains("NEVER_EXPORT"));
    assert!(parsed.flows[0].inputs[0].default.is_none());
    assert!(bus
        .workspace_bundle_preview(&"x".repeat(MAX_BUNDLE_BYTES + 1))
        .await
        .is_err());
}
