-- Keep unresolved legacy records intact. Never infer from SQL text or tree order.
ALTER TABLE saved_sql ADD COLUMN catalog TEXT;
ALTER TABLE saved_sql ADD COLUMN schema TEXT;
ALTER TABLE db_query_history ADD COLUMN catalog TEXT;
ALTER TABLE db_query_history ADD COLUMN schema TEXT;

UPDATE saved_sql
SET catalog = (
  SELECT NULLIF(TRIM(d.database_name), '')
  FROM database_connections d JOIN connections c ON c.id = d.connection_id
  WHERE c.id = saved_sql.connection_id AND c.workspace_id = saved_sql.workspace_id
    AND d.driver IN ('postgres', 'mysql')
);

UPDATE db_query_history
SET catalog = (
  SELECT NULLIF(TRIM(d.database_name), '')
  FROM database_connections d JOIN connections c ON c.id = d.connection_id
  WHERE c.id = db_query_history.connection_id AND c.workspace_id = db_query_history.workspace_id
    AND d.driver IN ('postgres', 'mysql')
);
