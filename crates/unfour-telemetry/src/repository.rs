use unfour_core::{AppError, AppResult};
use unfour_local_storage::LocalDb;

const ENABLED_KEY: &str = "telemetry.enabled";
const NOTICE_SHOWN_KEY: &str = "telemetry.first_notice_shown";
const LAST_SUCCESSFUL_ACTIVE_DATE_KEY: &str = "telemetry.last_successful_active_utc_date";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StoredTelemetryPreferences {
    pub(crate) enabled: bool,
    pub(crate) notice_shown: bool,
}

#[derive(Clone)]
pub(crate) struct TelemetryRepository {
    db: LocalDb,
}

impl TelemetryRepository {
    pub(crate) fn new(db: LocalDb) -> Self {
        Self { db }
    }

    pub(crate) async fn preferences(&self) -> AppResult<StoredTelemetryPreferences> {
        Ok(StoredTelemetryPreferences {
            enabled: self.boolean_setting(ENABLED_KEY, true).await?,
            notice_shown: self.boolean_setting(NOTICE_SHOWN_KEY, false).await?,
        })
    }

    pub(crate) async fn set_enabled(&self, enabled: bool) -> AppResult<()> {
        self.write_setting(ENABLED_KEY, if enabled { "true" } else { "false" })
            .await
    }

    pub(crate) async fn mark_notice_shown(&self) -> AppResult<()> {
        self.write_setting(NOTICE_SHOWN_KEY, "true").await
    }

    pub(crate) async fn last_successful_active_utc_date(&self) -> AppResult<Option<String>> {
        self.read_setting(LAST_SUCCESSFUL_ACTIVE_DATE_KEY).await
    }

    pub(crate) async fn set_last_successful_active_utc_date(
        &self,
        utc_date: &str,
    ) -> AppResult<()> {
        self.write_setting(LAST_SUCCESSFUL_ACTIVE_DATE_KEY, utc_date)
            .await
    }

    async fn boolean_setting(&self, key: &str, default: bool) -> AppResult<bool> {
        match self.read_setting(key).await?.as_deref() {
            None => Ok(default),
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            Some(_) => Err(AppError::Config(format!(
                "stored telemetry preference {key} is invalid"
            ))),
        }
    }

    async fn read_setting(&self, key: &str) -> AppResult<Option<String>> {
        Ok(
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = ?1")
                .bind(key)
                .fetch_optional(self.db.pool())
                .await?,
        )
    }

    async fn write_setting(&self, key: &str, value: &str) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            ON CONFLICT(key) DO UPDATE SET
              value = excluded.value,
              updated_at = excluded.updated_at
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }
}
