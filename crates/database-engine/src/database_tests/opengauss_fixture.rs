//! Canned protocol rows, not an implementation or certification of server SQL.
type Response = (Vec<(&'static str, u32)>, Vec<Vec<Option<Vec<u8>>>>);

pub(super) fn response(sql: &str, version: &str) -> Response {
    let text = |value: &str| Some(value.as_bytes().to_vec());
    if !version.contains("metadata fixture") || sql == "SELECT version()" {
        return (vec![("version", 25)], vec![vec![text(version)]]);
    }
    if sql.contains("AS primary_key") {
        return (
            vec![
                ("column_name", 25),
                ("data_type", 25),
                ("is_nullable", 25),
                ("column_default", 25),
                ("is_generated", 25),
                ("identity_generation", 25),
                ("primary_key", 16),
            ],
            vec![
                vec![
                    text("id"),
                    text("integer"),
                    text("NO"),
                    text("nextval('config_id_seq'::regclass)"),
                    text("NEVER"),
                    None,
                    Some(vec![1]),
                ],
                vec![
                    text("content"),
                    text("text"),
                    text("YES"),
                    None,
                    text("NEVER"),
                    None,
                    Some(vec![0]),
                ],
                vec![
                    text("computed"),
                    text("integer"),
                    text("YES"),
                    text("id + 1"),
                    text("ALWAYS"),
                    None,
                    Some(vec![0]),
                ],
            ],
        );
    }
    if sql.contains("FROM pg_database") {
        return (
            vec![("datname", 25)],
            vec![vec![text("qingqi_config")], vec![text("testdb")]],
        );
    }
    if sql.contains("SELECT table_schema, table_name, table_type") {
        return (
            vec![("table_schema", 25), ("table_name", 25), ("table_type", 25)],
            vec![vec![
                text("public"),
                text("config_info"),
                text("BASE TABLE"),
            ]],
        );
    }
    if sql.contains("SELECT table_name") {
        return (vec![("table_name", 25)], vec![vec![text("config_info")]]);
    }
    if sql.contains("SELECT table_type") {
        return (vec![("table_type", 25)], vec![vec![text("BASE TABLE")]]);
    }
    if sql.starts_with("SELECT \"content\" FROM \"public\".\"config_info\"") {
        return (vec![("content", 25)], vec![vec![text("selected content")]]);
    }
    if sql.starts_with("UPDATE ")
        || sql.starts_with("INSERT INTO ")
        || sql.starts_with("DELETE FROM ")
    {
        return (Vec::new(), Vec::new());
    }
    panic!("unsupported metadata/query must not be requested: {sql}");
}
