//! Writes only the database subtype row. Callers own validation, parent-row
//! revision/sync policy, credentials, mutations, and the transaction boundary.
//! Keep insert and update distinct: update must never repair a missing subtype.
use sqlx::{sqlite::SqliteQueryResult, SqliteConnection};
use unfour_core::AppResult;

pub(super) struct ConnectionSubtype<'a> {
    pub driver: &'a str,
    pub database_name: Option<&'a str>,
    pub username: Option<&'a str>,
    pub ssl_mode: Option<&'a str>,
    pub read_only: bool,
    pub config_json: &'a str,
}

impl ConnectionSubtype<'_> {
    pub async fn insert_on(
        &self,
        connection: &mut SqliteConnection,
        id: &str,
    ) -> AppResult<SqliteQueryResult> {
        Ok(sqlx::query(
            r#"
            INSERT INTO database_connections (
                connection_id, driver, database_name, username, ssl_mode, read_only, config_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(id)
        .bind(self.driver)
        .bind(self.database_name)
        .bind(self.username)
        .bind(self.ssl_mode)
        .bind(self.read_only)
        .bind(self.config_json)
        .execute(&mut *connection)
        .await?)
    }

    pub async fn update_on(
        &self,
        connection: &mut SqliteConnection,
        id: &str,
    ) -> AppResult<SqliteQueryResult> {
        Ok(sqlx::query(
            r#"
            UPDATE database_connections
            SET driver = ?1, database_name = ?2, username = ?3,
                ssl_mode = ?4, read_only = ?5, config_json = ?6
            WHERE connection_id = ?7
            "#,
        )
        .bind(self.driver)
        .bind(self.database_name)
        .bind(self.username)
        .bind(self.ssl_mode)
        .bind(self.read_only)
        .bind(self.config_json)
        .bind(id)
        .execute(&mut *connection)
        .await?)
    }
}
