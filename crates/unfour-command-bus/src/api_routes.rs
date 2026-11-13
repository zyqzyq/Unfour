use crate::CommandBus;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use unfour_core::{
    models::{ApiRequestInput, ApiResponse},
    network::{ApiRoute, ApiRouteEdit, ApiRoutes, ApiRoutingInput},
    AppError, AppResult,
};
use unfour_http_engine::local_routes;

impl CommandBus {
    pub async fn api_routes(
        &self,
        workspace: &str,
        request: Option<&str>,
        environment: Option<&str>,
    ) -> AppResult<ApiRoutes> {
        let environment = self
            .resolve_api_environment_id(workspace, environment)
            .await?;
        self.api_routes_for_environment(workspace, request, environment)
            .await
    }

    pub(crate) async fn api_routes_for_environment(
        &self,
        workspace: &str,
        request_id: Option<&str>,
        environment_id: Option<String>,
    ) -> AppResult<ApiRoutes> {
        let mut db = self.db.pool().begin().await?;
        let environment = match environment_id.as_deref() {
            Some(id) => local_routes::environment_route(&mut db, workspace, id).await?,
            None => ApiRoute::Direct,
        };
        let request = match request_id {
            Some(id) => local_routes::request_route(&mut db, workspace, id).await?,
            None => ApiRoute::Inherit,
        };
        db.commit().await?;
        let effective = effective_route(&request, &environment);
        Ok(ApiRoutes {
            request,
            environment,
            effective,
            environment_id,
        })
    }

    pub async fn set_api_environment_route(
        &self,
        workspace: &str,
        id: &str,
        edit: ApiRouteEdit,
    ) -> AppResult<()> {
        let mut tx = self.db.pool().begin().await?;
        local_routes::save_environment_route(&mut tx, workspace, id, &edit).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Check the editor's displayed jump before scripts can persist environment edits.
    /// Pin the same revision again when dialing, since scripts can outlive an edit.
    pub(super) async fn api_jump_revision(
        &self,
        workspace: &str,
        route: &ApiRoute,
        expected: Option<i64>,
    ) -> AppResult<Option<i64>> {
        let ApiRoute::Ssh { ssh_connection_id } = route else {
            return Ok(None);
        };
        let revision: i64 = sqlx::query_scalar(
            "SELECT revision FROM connections WHERE workspace_id=? AND id=?
             AND connection_type='ssh' AND deleted_at IS NULL",
        )
        .bind(workspace)
        .bind(ssh_connection_id)
        .fetch_optional(self.db.pool())
        .await?
        .ok_or_else(|| AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into()))?;
        if expected.is_some_and(|expected| expected != revision) {
            return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
        }
        Ok(Some(revision))
    }

    /// A request-scoped lease encloses redirect processing and the full response body.
    pub(crate) async fn send_routed_http(
        &self,
        input: ApiRequestInput,
        cancellation: CancellationToken,
        secrets: &[String],
        route: &ApiRoute,
        expected_ssh_revision: Option<i64>,
    ) -> AppResult<ApiResponse> {
        match route {
            ApiRoute::Direct => {
                self.api_client
                    .send_cancellable_with_secrets(input, cancellation, secrets)
                    .await
            }
            ApiRoute::Inherit => Err(AppError::Validation("SSH_TUNNEL_INVALID_ROUTE".into())),
            ApiRoute::Ssh { ssh_connection_id } => {
                let budget = input
                    .timeout_ms
                    .filter(|ms| *ms > 0)
                    .map(Duration::from_millis);
                let started = std::time::Instant::now();
                let connect = async {
                    let tunnel = self
                        .ssh
                        .open_tunnel_at_revision(
                            &input.workspace_id,
                            ssh_connection_id,
                            expected_ssh_revision,
                        )
                        .await
                        .map_err(|err| match err {
                            AppError::NotFound(_) => {
                                AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into())
                            }
                            other => other,
                        })?;
                    tunnel.http_proxy().await
                };
                let proxy = tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => return Err(AppError::ApiCancelled("cancelled by user".into())),
                    result = async {
                        match budget {
                            Some(limit) => tokio::time::timeout(limit, connect).await.map_err(|_| AppError::Validation("SSH_TUNNEL_TIMEOUT".into()))?,
                            None => connect.await,
                        }
                    } => result?,
                };
                self.api_client
                    .send_via_ssh_proxy(
                        input,
                        cancellation,
                        secrets,
                        proxy.address(),
                        proxy.username(),
                        proxy.password(),
                        started,
                    )
                    .await
                    .map_err(|err| match (&err, proxy.remote_failure_code()) {
                        (AppError::ApiNetwork(_), Some(code)) => AppError::Validation(code.into()),
                        _ => err,
                    })
            }
        }
    }
}

pub(super) fn checked_editor_route(
    saved: &ApiRoutes,
    preview: Option<&ApiRoutingInput>,
) -> AppResult<ApiRoute> {
    let Some(preview) = preview else {
        return Ok(saved.effective.clone());
    };
    let effective = effective_route(
        preview.route.as_ref().unwrap_or(&saved.request),
        &saved.environment,
    );
    if preview.expected_environment_id != saved.environment_id
        || effective != preview.expected_effective
    {
        return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
    }
    Ok(effective)
}

fn effective_route(request: &ApiRoute, environment: &ApiRoute) -> ApiRoute {
    match request {
        ApiRoute::Inherit => environment.clone(),
        other => other.clone(),
    }
}
