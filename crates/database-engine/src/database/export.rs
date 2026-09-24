use super::*;
use futures_util::TryStreamExt;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

const INSERT_BATCH_SIZE: usize = 100;

#[derive(Debug, Clone, PartialEq)]
enum ExportCell {
    Null,
    Text(String),
    Number(String),
    Bool(bool),
    Binary(Vec<u8>),
    Json(serde_json::Value),
}

impl DatabaseService {
    pub async fn export_table(
        &self,
        input: DatabaseExportTableInput,
    ) -> AppResult<DatabaseExportTableResult> {
        validate_workspace_id(&input.workspace_id)?;
        validate_connection_id(&input.connection_id)?;
        let table_name = input.table_name.trim();
        if table_name.is_empty() {
            return Err(AppError::Validation("table name cannot be empty".into()));
        }
        if !matches!(input.content, DatabaseExportContent::Data)
            && !matches!(input.format, DatabaseExportFormat::Sql)
        {
            return Err(AppError::Validation(
                "structure export requires SQL format".into(),
            ));
        }
        let path = PathBuf::from(&input.destination_path);
        if !path.is_absolute() || path.is_dir() {
            return Err(AppError::Validation(
                "export destination must be an absolute file path".into(),
            ));
        }
        let connection = self
            .get_connection(&input.workspace_id, &input.connection_id)
            .await?;
        if connection.driver == "sqlite"
            && connection.sqlite_path.as_deref().is_some_and(|source| {
                Path::new(source)
                    .canonicalize()
                    .ok()
                    .is_some_and(|source| path.canonicalize().ok().as_ref() == Some(&source))
            })
        {
            return Err(AppError::Validation(
                "export destination is the source database".into(),
            ));
        }
        let structure = self
            .table_structure(DatabaseTableStructureInput {
                workspace_id: input.workspace_id.clone(),
                connection_id: input.connection_id.clone(),
                catalog: input.catalog.clone(),
                schema: input.schema.clone(),
                table_name: table_name.into(),
            })
            .await?;
        if structure.kind != "table" {
            return Err(AppError::Unsupported("Only tables can be exported".into()));
        }
        let dialect = DatabaseDialect::for_driver(&connection.driver);
        let qualified = match dialect {
            DatabaseDialect::Postgres => quote_qualified_identifier(
                structure.schema.as_deref().unwrap_or("public"),
                table_name,
            ),
            DatabaseDialect::Mysql => quote_mysql_qualified_identifier(
                structure
                    .catalog
                    .as_deref()
                    .ok_or_else(|| AppError::Validation("MySQL catalog is required".into()))?,
                table_name,
            ),
            DatabaseDialect::Sqlite => quote_identifier(table_name),
            other => {
                return Err(AppError::Unsupported(format!(
                    "{other:?} table export is not supported"
                )))
            }
        };
        let staging = sibling_temp_path(&path);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        let mut cleanup = PartialExport {
            path: staging.clone(),
            committed: false,
        };
        let data_query = if matches!(input.content, DatabaseExportContent::Structure) {
            None
        } else {
            Some(build_export_select(
                dialect,
                &qualified,
                &structure.columns,
                input.columns.as_deref(),
                &input.filters,
                input.limit,
            )?)
        };
        let export_columns = data_query
            .as_ref()
            .map(|query| query.columns.clone())
            .unwrap_or_else(|| structure.columns.clone());
        let mut writer = ExportWriter::new(
            BufWriter::new(file),
            input.format.clone(),
            dialect,
            qualified,
            &export_columns,
        )?;
        if !matches!(input.content, DatabaseExportContent::Data) {
            let ddl = structure.ddl.as_deref().ok_or_else(|| {
                AppError::Unsupported("DDL is not available for this table".into())
            })?;
            writer.write_ddl(ddl)?;
        }
        if let Some(data_query) = data_query {
            let sql = data_query.sql;
            let binds = data_query.binds;
            match connection.driver.as_str() {
                "sqlite" => {
                    let pool = sqlite_pool(&connection).await?;
                    require_capability(pool.profile.capabilities.export, "table export")?;
                    let mut query = sqlx::query(&sql);
                    for value in &binds {
                        query = query.bind(value);
                    }
                    let mut rows = query.fetch(&*pool);
                    while let Some(row) = rows.try_next().await? {
                        writer.write_row(sqlite_export_values(&row)?)?;
                    }
                }
                "postgres" => {
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.postgres_pool(&effective).await?;
                    require_capability(pool.profile.capabilities.export, "table export")?;
                    let mut query = sqlx::query(&sql);
                    for value in &binds {
                        query = query.bind(value);
                    }
                    let mut rows = query.fetch(&*pool);
                    while let Some(row) = rows.try_next().await.map_err(sanitize_pg_error)? {
                        writer.write_row(postgres_export_values(&row)?)?;
                    }
                }
                "mysql" => {
                    let effective =
                        Self::effective_connection(&connection, input.catalog.as_deref());
                    let pool = self.mysql_pool(&effective).await?;
                    require_capability(pool.profile.capabilities.export, "table export")?;
                    let mut query = sqlx::query(&sql);
                    for value in &binds {
                        query = query.bind(value);
                    }
                    let mut rows = query.fetch(&*pool);
                    while let Some(row) = rows.try_next().await.map_err(sanitize_mysql_error)? {
                        writer.write_row(mysql_export_values(&row)?)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        let row_count = writer.finish()?;
        let bytes_written = fs::metadata(&staging)?.len();
        replace_export(&staging, &path)?;
        cleanup.committed = true;
        Ok(DatabaseExportTableResult {
            path: path.to_string_lossy().into_owned(),
            row_count,
            bytes_written,
            format: input.format,
        })
    }
}

fn sibling_temp_path(destination: &Path) -> PathBuf {
    destination.with_file_name(format!(".unfour-export-{}.tmp", uuid::Uuid::new_v4()))
}

fn replace_export(staging: &Path, destination: &Path) -> std::io::Result<()> {
    if !destination.exists() {
        return fs::rename(staging, destination);
    }
    // Windows cannot rename over an existing file. Move the previous export
    // aside and restore it if the final rename fails.
    let backup = sibling_temp_path(destination);
    fs::rename(destination, &backup)?;
    match fs::rename(staging, destination) {
        Ok(()) => {
            let _ = fs::remove_file(backup);
            Ok(())
        }
        Err(error) => {
            let _ = fs::rename(backup, destination);
            Err(error)
        }
    }
}

struct PartialExport {
    path: PathBuf,
    committed: bool,
}

impl Drop for PartialExport {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

struct ExportWriter<W: Write> {
    out: W,
    format: DatabaseExportFormat,
    dialect: DatabaseDialect,
    table: String,
    columns: Vec<DatabaseTableColumn>,
    insert_columns: Vec<usize>,
    insert_batch: Vec<String>,
    row_count: u64,
}

impl<W: Write> ExportWriter<W> {
    fn new(
        out: W,
        format: DatabaseExportFormat,
        dialect: DatabaseDialect,
        table: String,
        columns: &[DatabaseTableColumn],
    ) -> AppResult<Self> {
        let mut this = Self {
            out,
            format,
            dialect,
            table,
            columns: columns.to_vec(),
            insert_columns: columns
                .iter()
                .enumerate()
                .filter_map(|(i, c)| (!c.generated || c.auto_increment).then_some(i))
                .collect(),
            insert_batch: Vec::with_capacity(INSERT_BATCH_SIZE),
            row_count: 0,
        };
        match this.format {
            DatabaseExportFormat::Csv => {
                this.write_csv_record(&columns.iter().map(|c| c.name.clone()).collect::<Vec<_>>())?
            }
            DatabaseExportFormat::Json => this.out.write_all(b"[\n")?,
            DatabaseExportFormat::Sql => {}
        }
        Ok(this)
    }

    fn write_ddl(&mut self, ddl: &str) -> AppResult<()> {
        self.out.write_all(ddl.as_bytes())?;
        if !ddl.trim_end().ends_with(';') {
            self.out.write_all(b";")?;
        }
        self.out.write_all(b"\n")?;
        Ok(())
    }

    fn write_row(&mut self, values: Vec<ExportCell>) -> AppResult<()> {
        if values.len() != self.columns.len() {
            return Err(AppError::Validation(
                "table columns changed during export".into(),
            ));
        }
        match self.format {
            DatabaseExportFormat::Csv => {
                self.write_csv_record(&values.iter().map(cell_text).collect::<Vec<_>>())?;
            }
            DatabaseExportFormat::Json => {
                if self.row_count > 0 {
                    self.out.write_all(b",\n")?;
                }
                let object: serde_json::Map<String, serde_json::Value> = self
                    .columns
                    .iter()
                    .zip(values.iter())
                    .map(|(column, value)| (column.name.clone(), cell_json(value)))
                    .collect();
                serde_json::to_writer(&mut self.out, &object)
                    .map_err(|e| AppError::Validation(e.to_string()))?;
            }
            DatabaseExportFormat::Sql => {
                if self.insert_columns.is_empty() {
                    self.out.write_all(
                        format!("INSERT INTO {} DEFAULT VALUES;\n", self.table).as_bytes(),
                    )?;
                } else {
                    let tuple = self
                        .insert_columns
                        .iter()
                        .map(|i| sql_literal(&values[*i], self.dialect))
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.insert_batch.push(format!("({tuple})"));
                    if self.insert_batch.len() == INSERT_BATCH_SIZE {
                        self.flush_inserts()?;
                    }
                }
            }
        }
        self.row_count += 1;
        Ok(())
    }

    fn write_csv_record(&mut self, fields: &[String]) -> AppResult<()> {
        for (i, field) in fields.iter().enumerate() {
            if i != 0 {
                self.out.write_all(b",")?;
            }
            let quote = field
                .chars()
                .any(|ch| matches!(ch, ',' | '"' | '\n' | '\r'));
            if quote {
                self.out.write_all(b"\"")?;
            }
            self.out.write_all(field.replace('"', "\"\"").as_bytes())?;
            if quote {
                self.out.write_all(b"\"")?;
            }
        }
        self.out.write_all(b"\r\n")?;
        Ok(())
    }

    fn flush_inserts(&mut self) -> AppResult<()> {
        if self.insert_batch.is_empty() {
            return Ok(());
        }
        let quote = |s: &str| {
            if self.dialect == DatabaseDialect::Mysql {
                quote_mysql_identifier(s)
            } else {
                quote_identifier(s)
            }
        };
        let columns = self
            .insert_columns
            .iter()
            .map(|i| quote(&self.columns[*i].name))
            .collect::<Vec<_>>()
            .join(", ");
        let override_identity = if self.dialect == DatabaseDialect::Postgres
            && self
                .insert_columns
                .iter()
                .any(|i| self.columns[*i].generated && self.columns[*i].auto_increment)
        {
            " OVERRIDING SYSTEM VALUE"
        } else {
            ""
        };
        writeln!(
            self.out,
            "INSERT INTO {} ({columns}){override_identity} VALUES\n{};",
            self.table,
            self.insert_batch.join(",\n")
        )?;
        self.insert_batch.clear();
        Ok(())
    }

    fn finish(mut self) -> AppResult<u64> {
        if matches!(self.format, DatabaseExportFormat::Sql) {
            self.flush_inserts()?;
        }
        if matches!(self.format, DatabaseExportFormat::Json) {
            self.out.write_all(b"\n]\n")?;
        }
        self.out.flush()?;
        Ok(self.row_count)
    }
}

fn cell_text(cell: &ExportCell) -> String {
    match cell {
        ExportCell::Null => String::new(),
        ExportCell::Text(s) | ExportCell::Number(s) => s.clone(),
        ExportCell::Bool(b) => b.to_string(),
        ExportCell::Binary(bytes) => hex(bytes),
        ExportCell::Json(value) => value.to_string(),
    }
}

fn cell_json(cell: &ExportCell) -> serde_json::Value {
    match cell {
        ExportCell::Null => serde_json::Value::Null,
        ExportCell::Text(s) => serde_json::Value::String(s.clone()),
        ExportCell::Number(s) => {
            serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.clone()))
        }
        ExportCell::Bool(b) => serde_json::Value::Bool(*b),
        ExportCell::Binary(bytes) => serde_json::Value::String(hex(bytes)),
        ExportCell::Json(value) => value.clone(),
    }
}

fn sql_literal(cell: &ExportCell, dialect: DatabaseDialect) -> String {
    match cell {
        ExportCell::Null => "NULL".into(),
        ExportCell::Number(value) => value.clone(),
        ExportCell::Bool(value) => match dialect {
            DatabaseDialect::Postgres => value.to_string().to_uppercase(),
            _ => {
                if *value {
                    "1".into()
                } else {
                    "0".into()
                }
            }
        },
        ExportCell::Binary(bytes) => match dialect {
            DatabaseDialect::Postgres => format!("decode('{}', 'hex')", hex(bytes)),
            _ => format!("X'{}'", hex(bytes)),
        },
        ExportCell::Text(value) => quote_sql_text(value, dialect),
        ExportCell::Json(value) => quote_sql_text(&value.to_string(), dialect),
    }
}

fn quote_sql_text(value: &str, dialect: DatabaseDialect) -> String {
    if dialect == DatabaseDialect::Sqlite && value.contains('\0') {
        return format!("CAST(X'{}' AS TEXT)", hex(value.as_bytes()));
    }
    if dialect == DatabaseDialect::Mysql && value.chars().any(|ch| ch == '\\' || ch.is_control()) {
        // Hex conversion is independent of NO_BACKSLASH_ESCAPES and preserves
        // newlines and NUL bytes without relying on the target session mode.
        return format!("CONVERT(X'{}' USING utf8mb4)", hex(value.as_bytes()));
    }
    let quoted = value.replace('\'', "''");
    match dialect {
        DatabaseDialect::Postgres => format!("E'{}'", quoted.replace('\\', "\\\\")),
        DatabaseDialect::Mysql => format!("'{quoted}'"),
        _ => format!("'{quoted}'"),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 15) as usize] as char);
    }
    output
}

fn sqlite_export_values(row: &sqlx::sqlite::SqliteRow) -> AppResult<Vec<ExportCell>> {
    (0..row.columns().len())
        .map(|i| {
            let raw = row.try_get_raw(i)?;
            if raw.is_null() {
                return Ok(ExportCell::Null);
            }
            match raw.type_info().name() {
                "BLOB" => Ok(ExportCell::Binary(row.try_get(i)?)),
                "INTEGER" => Ok(ExportCell::Number(row.try_get::<i64, _>(i)?.to_string())),
                "REAL" => Ok(ExportCell::Number(row.try_get::<f64, _>(i)?.to_string())),
                _ => Ok(ExportCell::Text(row.try_get(i)?)),
            }
        })
        .collect()
}

fn postgres_export_values(row: &sqlx::postgres::PgRow) -> AppResult<Vec<ExportCell>> {
    (0..row.columns().len())
        .map(|i| {
            if row.try_get_raw(i)?.is_null() {
                return Ok(ExportCell::Null);
            }
            let kind = row.columns()[i].type_info().name().to_ascii_uppercase();
            if kind == "BYTEA" {
                return Ok(ExportCell::Binary(row.try_get(i)?));
            }
            if kind == "BOOL" {
                return Ok(ExportCell::Bool(row.try_get(i)?));
            }
            if kind == "JSON" || kind == "JSONB" {
                return Ok(ExportCell::Json(row.try_get(i)?));
            }
            if let Ok(v) = row.try_get::<i64, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<i32, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<i16, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<sqlx::types::BigDecimal, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<f64, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<f32, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<String, _>(i) {
                return Ok(ExportCell::Text(v));
            }
            if let Ok(v) = row.try_get::<uuid::Uuid, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            if let Ok(v) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(i) {
                return Ok(ExportCell::Text(v.to_rfc3339()));
            }
            if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            if let Ok(v) = row.try_get::<chrono::NaiveTime, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            Err(AppError::Unsupported(format!(
                "export of PostgreSQL type {kind} is not supported"
            )))
        })
        .collect()
}

fn mysql_export_values(row: &sqlx::mysql::MySqlRow) -> AppResult<Vec<ExportCell>> {
    (0..row.columns().len())
        .map(|i| {
            if row.try_get_raw(i)?.is_null() {
                return Ok(ExportCell::Null);
            }
            let kind = row.columns()[i].type_info().name().to_ascii_uppercase();
            if kind.contains("BLOB") || kind.contains("BINARY") {
                return Ok(ExportCell::Binary(row.try_get(i)?));
            }
            if kind == "JSON" {
                return Ok(ExportCell::Json(row.try_get(i)?));
            }
            if let Ok(v) = row.try_get::<i64, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<u64, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<sqlx::types::BigDecimal, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<f64, _>(i) {
                return Ok(ExportCell::Number(v.to_string()));
            }
            if let Ok(v) = row.try_get::<String, _>(i) {
                return Ok(ExportCell::Text(v));
            }
            if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            if let Ok(v) = row.try_get::<chrono::NaiveDate, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            if let Ok(v) = row.try_get::<chrono::NaiveTime, _>(i) {
                return Ok(ExportCell::Text(v.to_string()));
            }
            Err(AppError::Unsupported(format!(
                "export of MySQL type {kind} is not supported"
            )))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str) -> DatabaseTableColumn {
        DatabaseTableColumn {
            name: name.into(),
            data_type: "text".into(),
            nullable: true,
            primary_key: false,
            default_value: None,
            generated: false,
            auto_increment: false,
        }
    }

    #[test]
    fn csv_json_sql_escape_and_empty_table() {
        let columns = vec![column("name"), column("payload")];
        let cells = vec![ExportCell::Text("a,\"b\nline".into()), ExportCell::Null];
        let mut csv = Vec::new();
        let mut writer = ExportWriter::new(
            &mut csv,
            DatabaseExportFormat::Csv,
            DatabaseDialect::Sqlite,
            "\"items\"".into(),
            &columns,
        )
        .unwrap();
        writer.write_row(cells.clone()).unwrap();
        assert_eq!(writer.finish().unwrap(), 1);
        assert_eq!(
            String::from_utf8(csv).unwrap(),
            "name,payload\r\n\"a,\"\"b\nline\",\r\n"
        );

        let mut json = Vec::new();
        let writer = ExportWriter::new(
            &mut json,
            DatabaseExportFormat::Json,
            DatabaseDialect::Sqlite,
            "\"items\"".into(),
            &columns,
        )
        .unwrap();
        assert_eq!(writer.finish().unwrap(), 0);
        assert_eq!(String::from_utf8(json).unwrap(), "[\n\n]\n");

        let mut sql = Vec::new();
        let mut writer = ExportWriter::new(
            &mut sql,
            DatabaseExportFormat::Sql,
            DatabaseDialect::Sqlite,
            "\"items\"".into(),
            &columns,
        )
        .unwrap();
        writer
            .write_row(vec![
                ExportCell::Text("O'Brien".into()),
                ExportCell::Binary(vec![0, 255]),
            ])
            .unwrap();
        writer.finish().unwrap();
        assert!(String::from_utf8(sql)
            .unwrap()
            .contains("('O''Brien', X'00ff')"));
        assert_eq!(
            sql_literal(&ExportCell::Bool(true), DatabaseDialect::Postgres),
            "TRUE"
        );
        assert_eq!(
            sql_literal(&ExportCell::Number("12.50".into()), DatabaseDialect::Mysql),
            "12.50"
        );
        assert_eq!(
            sql_literal(&ExportCell::Null, DatabaseDialect::Sqlite),
            "NULL"
        );
        assert_eq!(
            cell_json(&ExportCell::Number(
                "12345678901234567890.1234567890".into()
            ))
            .to_string(),
            "12345678901234567890.1234567890"
        );
        assert_eq!(
            sql_literal(
                &ExportCell::Json(serde_json::json!({"a":"b"})),
                DatabaseDialect::Sqlite
            ),
            "'{\"a\":\"b\"}'"
        );
        assert_eq!(
            quote_sql_text("a\\b'c", DatabaseDialect::Postgres),
            "E'a\\\\b''c'"
        );
        assert_eq!(
            quote_sql_text("a\0b", DatabaseDialect::Sqlite),
            "CAST(X'610062' AS TEXT)"
        );
        assert_eq!(
            quote_sql_text("a\\b", DatabaseDialect::Mysql),
            "CONVERT(X'615c62' USING utf8mb4)"
        );
    }

    #[test]
    fn sql_batches_are_bounded() {
        let mut output = Vec::new();
        let mut writer = ExportWriter::new(
            &mut output,
            DatabaseExportFormat::Sql,
            DatabaseDialect::Sqlite,
            "\"items\"".into(),
            &[column("id")],
        )
        .unwrap();
        for i in 0..250 {
            writer
                .write_row(vec![ExportCell::Number(i.to_string())])
                .unwrap();
        }
        assert_eq!(writer.finish().unwrap(), 250);
        let sql = String::from_utf8(output).unwrap();
        assert_eq!(sql.matches("INSERT INTO").count(), 3);
        assert_eq!(sql.matches(");").count(), 3);
    }

    #[test]
    fn completed_export_replaces_existing_file() {
        let destination = std::env::temp_dir().join(format!(
            "unfour-existing-export-{}.sql",
            uuid::Uuid::new_v4()
        ));
        let staging = sibling_temp_path(&destination);
        fs::write(&destination, "old export").unwrap();
        fs::write(&staging, "new export").unwrap();
        replace_export(&staging, &destination).unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), "new export");
        assert!(!staging.exists());
        let _ = fs::remove_file(destination);
    }

    #[test]
    fn export_select_quotes_columns_binds_in_filters_and_limit() {
        let columns = vec![column("data_id"), column("name")];
        let filters = vec![DatabaseExportFilter {
            column: "data_id".into(),
            op: DatabaseExportFilterOp::In,
            values: vec![Some("10".into()), Some("20".into())],
        }];
        let sqlite = build_export_select(
            DatabaseDialect::Sqlite,
            "\"items\"",
            &columns,
            Some(&["name".into()]),
            &filters,
            Some(5),
        )
        .unwrap();
        assert_eq!(
            sqlite.sql,
            "SELECT \"name\" FROM \"items\" WHERE CAST(\"data_id\" AS TEXT) IN (?, ?) LIMIT 5"
        );
        assert_eq!(sqlite.binds, vec!["10".to_string(), "20".to_string()]);
        assert_eq!(sqlite.columns.len(), 1);

        let postgres = build_export_select(
            DatabaseDialect::Postgres,
            "\"public\".\"items\"",
            &columns,
            None,
            &[DatabaseExportFilter {
                column: "name".into(),
                op: DatabaseExportFilterOp::Eq,
                values: vec![Some("api".into())],
            }],
            None,
        )
        .unwrap();
        assert_eq!(
            postgres.sql,
            "SELECT * FROM \"public\".\"items\" WHERE CAST(\"name\" AS TEXT) = $1"
        );
        assert_eq!(postgres.columns.len(), 2);

        let mysql = build_export_select(
            DatabaseDialect::Mysql,
            "`app`.`items`",
            &columns,
            None,
            &[DatabaseExportFilter {
                column: "data_id".into(),
                op: DatabaseExportFilterOp::Eq,
                values: vec![None],
            }],
            Some(1),
        )
        .unwrap();
        assert_eq!(
            mysql.sql,
            "SELECT * FROM `app`.`items` WHERE `data_id` IS NULL LIMIT 1"
        );
        assert!(mysql.binds.is_empty());

        assert!(build_export_select(
            DatabaseDialect::Sqlite,
            "\"items\"",
            &columns,
            Some(&["missing".into()]),
            &[],
            None,
        )
        .is_err());
        assert!(build_export_select(
            DatabaseDialect::Sqlite,
            "\"items\"",
            &columns,
            None,
            &[DatabaseExportFilter {
                column: "data_id".into(),
                op: DatabaseExportFilterOp::In,
                values: vec![None],
            }],
            None,
        )
        .is_err());
    }
}
