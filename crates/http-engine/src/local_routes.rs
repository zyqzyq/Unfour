//! Device-local transport preferences. Portable API definitions and history never contain them.
use sqlx::SqliteConnection;
use unfour_core::{
    network::{ApiRoute, ApiRouteEdit},
    AppError, AppResult,
};

fn missing() -> AppError {
    AppError::Validation("SSH_TUNNEL_ROUTE_MISSING".into())
}

async fn check_ssh(db: &mut SqliteConnection, ws: &str, route: &ApiRoute) -> AppResult<()> {
    if let ApiRoute::Ssh { ssh_connection_id } = route {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM connections WHERE workspace_id=? AND id=? AND connection_type='ssh' AND deleted_at IS NULL)")
            .bind(ws).bind(ssh_connection_id).fetch_one(db).await?;
        if !exists {
            return Err(missing());
        }
    }
    Ok(())
}

pub async fn request_route(db: &mut SqliteConnection, ws: &str, id: &str) -> AppResult<ApiRoute> {
    let (owner, ssh): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT r.request_id,r.ssh_connection_id FROM api_requests a LEFT JOIN api_request_local_routes r
         ON r.workspace_id=a.workspace_id AND r.request_id=a.id WHERE a.workspace_id=? AND a.id=? AND a.deleted_at IS NULL")
        .bind(ws).bind(id).fetch_optional(db).await?.ok_or_else(missing)?;
    Ok(match (owner, ssh) {
        (_, Some(ssh_connection_id)) => ApiRoute::Ssh { ssh_connection_id },
        (Some(_), None) => ApiRoute::Direct,
        _ => ApiRoute::Inherit,
    })
}

pub async fn environment_route(
    db: &mut SqliteConnection,
    ws: &str,
    id: &str,
) -> AppResult<ApiRoute> {
    let ssh: Option<String> = sqlx::query_scalar(
        "SELECT r.ssh_connection_id FROM workspace_environments e LEFT JOIN api_environment_local_routes r
         ON r.workspace_id=e.workspace_id AND r.environment_id=e.id WHERE e.workspace_id=? AND e.id=? AND e.deleted_at IS NULL")
        .bind(ws).bind(id).fetch_optional(db).await?.ok_or_else(missing)?;
    Ok(
        ssh.map_or(ApiRoute::Direct, |ssh_connection_id| ApiRoute::Ssh {
            ssh_connection_id,
        }),
    )
}

pub async fn save_request_route(
    db: &mut SqliteConnection,
    ws: &str,
    id: &str,
    route: &ApiRoute,
) -> AppResult<()> {
    check_ssh(db, ws, route).await?;
    let ssh = match route {
        ApiRoute::Inherit => {
            sqlx::query(
                "DELETE FROM api_request_local_routes WHERE workspace_id=? AND request_id=?",
            )
            .bind(ws)
            .bind(id)
            .execute(db)
            .await?;
            return Ok(());
        }
        ApiRoute::Direct => None,
        ApiRoute::Ssh { ssh_connection_id } => Some(ssh_connection_id),
    };
    sqlx::query("INSERT INTO api_request_local_routes VALUES(?,?,?) ON CONFLICT(workspace_id,request_id) DO UPDATE SET ssh_connection_id=excluded.ssh_connection_id")
        .bind(ws).bind(id).bind(ssh).execute(db).await?;
    Ok(())
}

pub async fn save_environment_route(
    db: &mut SqliteConnection,
    ws: &str,
    id: &str,
    edit: &ApiRouteEdit,
) -> AppResult<()> {
    ensure_expected(environment_route(db, ws, id).await?, edit)?;
    let route = &edit.route;
    check_ssh(db, ws, route).await?;
    match route {
        ApiRoute::Ssh{ssh_connection_id} => sqlx::query("INSERT INTO api_environment_local_routes VALUES(?,?,?) ON CONFLICT(workspace_id,environment_id) DO UPDATE SET ssh_connection_id=excluded.ssh_connection_id")
            .bind(ws).bind(id).bind(ssh_connection_id).execute(db).await?,
        ApiRoute::Direct => sqlx::query("DELETE FROM api_environment_local_routes WHERE workspace_id=? AND environment_id=?")
            .bind(ws).bind(id).execute(db).await?,
        ApiRoute::Inherit => return Err(AppError::Validation("SSH_TUNNEL_INVALID_ROUTE".into())),
    };
    Ok(())
}

pub async fn update_request_route(
    db: &mut SqliteConnection,
    ws: &str,
    id: &str,
    edit: &ApiRouteEdit,
) -> AppResult<()> {
    ensure_expected(request_route(db, ws, id).await?, edit)?;
    save_request_route(db, ws, id, &edit.route).await
}

fn ensure_expected(current: ApiRoute, edit: &ApiRouteEdit) -> AppResult<()> {
    if current != edit.expected {
        return Err(AppError::Validation("SSH_TUNNEL_ROUTE_CHANGED".into()));
    }
    Ok(())
}
