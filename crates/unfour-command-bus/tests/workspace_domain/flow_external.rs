use super::*;
use serde_json::json;
use unfour_core::domain::{ExternalFlowDefinitionApply, ExternalFlowDefinitionUpsert};

#[tokio::test]
async fn flow_external_apply_hooks_are_external_and_failure_rolls_back_the_page() {
    for reject in [false, true] {
        let (bus, db) = bus_with_hooks(vec![
            Arc::new(SqlHook {
                name: "local-only",
                fail_on: None,
                local_only: true,
            }),
            Arc::new(SqlHook {
                name: "observer",
                fail_on: reject.then_some("workspace.external.apply_page"),
                local_only: false,
            }),
        ])
        .await;
        let ws = bus.list_workspaces().await.unwrap().active_workspace_id;
        let id = "00000000-0000-4000-8000-000000000001";
        let definition = json!({"id":id,"workspaceId":ws,"revision":99,"name":"Remote","inputs":[],
            "steps":[{"id":"wait","name":"Wait","kind":"wait","durationMs":0,"timeoutMs":1000}]});
        let page = ExternalApplyPage {
            flow_definitions: vec![ExternalFlowDefinitionApply::Upsert(
                ExternalFlowDefinitionUpsert {
                    id: id.into(),
                    workspace_id: ws.clone(),
                    definition,
                },
            )],
            ..Default::default()
        };
        let result = bus.apply_external_page(page.clone()).await;
        if reject {
            assert!(result.is_err());
            assert!(bus.get_flow(ws.clone(), id.into()).await.is_err());
        } else {
            let report = result.unwrap();
            assert_eq!(report.applied_count, 1);
            assert_eq!(report.mutations[0].origin, MutationOrigin::External);
            assert_eq!(bus.get_flow(ws, id.into()).await.unwrap().revision, 1);
            let duplicate = bus.apply_external_page(page).await.unwrap();
            assert_eq!(duplicate.applied_count, 0);
            assert_eq!(duplicate.materialized_entities.len(), 1);
        }
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT hook_name,origin FROM hook_effects")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(
            rows,
            if reject {
                vec![]
            } else {
                vec![("observer".into(), "External".into())]
            }
        );
    }
}
