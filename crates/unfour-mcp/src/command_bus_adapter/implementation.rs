// Typed domain forwarding, separate from runtime construction and shutdown.
use super::*;

impl CommandBusAdapter for LocalCommandBusAdapter {
    fn api_folder_delete_snapshot(
        &self,
        workspace_id: String,
        folder_id: String,
    ) -> Result<unfour_core::models::ApiFolderDeleteSnapshot, CommandBusAdapterError> {
        self.run(self.bus.api_folder_delete_snapshot(workspace_id, folder_id))
            .map_err(|error| {
                CommandBusAdapterError::from_app_error(
                    "The folder deletion preview failed.",
                    &error,
                )
            })
    }
    fn create_workspace_inactive(
        &self,
        name: String,
        source_id: String,
        environment_type: String,
        mcp_policy: String,
    ) -> Result<Workspace, CommandBusAdapterError> {
        self.run(self.bus.create_workspace_inactive_from(
            name,
            source_id,
            environment_type,
            mcp_policy,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn rename_workspace(
        &self,
        workspace_id: String,
        name: String,
        expected_name: String,
    ) -> Result<Workspace, CommandBusAdapterError> {
        self.run(
            self.bus
                .rename_workspace_checked(workspace_id, name, Some(expected_name)),
        )
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn list_api_folders(
        &self,
        workspace_id: String,
        collection_id: Option<String>,
    ) -> Result<Vec<ApiCollectionFolder>, CommandBusAdapterError> {
        self.run(
            self.bus
                .api_collection_folders_list(workspace_id, collection_id),
        )
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn create_api_folder(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        name: String,
    ) -> Result<ApiCollectionFolder, CommandBusAdapterError> {
        self.run(self.bus.api_collection_folder_create(
            workspace_id,
            collection_id,
            parent_folder_id,
            name,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn rename_api_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        name: String,
        expected_updated_at: String,
    ) -> Result<ApiCollectionFolder, CommandBusAdapterError> {
        self.run(self.bus.api_collection_folder_rename_if_current(
            workspace_id,
            folder_id,
            name,
            Some(expected_updated_at),
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn move_api_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        parent_folder_id: Option<String>,
    ) -> Result<ApiCollectionFolder, CommandBusAdapterError> {
        self.run(
            self.bus
                .api_collection_folder_move(workspace_id, folder_id, parent_folder_id),
        )
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn delete_api_folder(
        &self,
        workspace_id: String,
        folder_id: String,
        expected: unfour_core::models::ApiFolderDeleteSnapshot,
    ) -> Result<Vec<ApiCollectionFolder>, CommandBusAdapterError> {
        self.run(self.bus.api_collection_folder_delete_checked(
            workspace_id,
            folder_id,
            Some(expected),
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn reorder_api_folders(
        &self,
        workspace_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
        folder_ids: Vec<String>,
    ) -> Result<Vec<ApiCollectionFolder>, CommandBusAdapterError> {
        self.run(self.bus.api_collection_folders_reorder(
            workspace_id,
            collection_id,
            parent_folder_id,
            folder_ids,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn move_api_request(
        &self,
        workspace_id: String,
        request_id: String,
        collection_id: String,
        parent_folder_id: Option<String>,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.run(self.bus.api_request_move(
            workspace_id,
            request_id,
            Some(collection_id),
            parent_folder_id,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn duplicate_api_request(
        &self,
        workspace_id: String,
        request_id: String,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.run(self.bus.duplicate_api_request(workspace_id, request_id))
            .map_err(|error| {
                CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
            })
    }
    fn list_saved_sql(
        &self,
        workspace_id: String,
    ) -> Result<Vec<SavedSql>, CommandBusAdapterError> {
        self.run(self.bus.list_saved_sql(workspace_id))
            .map_err(|error| {
                CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
            })
    }
    fn save_saved_sql(
        &self,
        input: SavedSqlInput,
        expected_revision: Option<i64>,
    ) -> Result<SavedSql, CommandBusAdapterError> {
        self.run(self.bus.save_saved_sql_checked(input, expected_revision))
            .map_err(|error| {
                CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
            })
    }
    fn delete_saved_sql(
        &self,
        workspace_id: String,
        id: String,
        expected_revision: i64,
    ) -> Result<Vec<SavedSql>, CommandBusAdapterError> {
        self.run(
            self.bus
                .delete_saved_sql_checked(workspace_id, id, Some(expected_revision)),
        )
        .map_err(|error| {
            CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
        })
    }
    fn list_database_catalogs(
        &self,
        workspace_id: String,
        connection_id: String,
    ) -> Result<Vec<String>, CommandBusAdapterError> {
        self.run(self.bus.database_catalogs(workspace_id, connection_id))
            .map_err(|error| {
                CommandBusAdapterError::from_app_error("The metadata operation failed.", &error)
            })
    }
    fn get_db_table_structure(
        &self,
        input: DatabaseTableStructureInput,
    ) -> Result<DatabaseTableStructure, CommandBusAdapterError> {
        self.run_execution(self.bus.database_table_structure(input))
            .map_err(|e| {
                CommandBusAdapterError::from_database_app_error(
                    "The table structure operation failed.",
                    &e,
                )
            })
    }

    fn export_db_table(
        &self,
        input: DatabaseExportTableInput,
    ) -> Result<DatabaseExportTableResult, CommandBusAdapterError> {
        self.run_execution(self.bus.database_export_table(input))
            .map_err(|e| {
                CommandBusAdapterError::from_database_app_error(
                    "The table export operation failed.",
                    &e,
                )
            })
    }
    fn list_flows(&self, workspace_id: &str) -> Result<Vec<FlowSummary>, CommandBusAdapterError> {
        self.run(self.bus.list_flow_summaries(workspace_id.to_string()))
            .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn get_flow(
        &self,
        workspace_id: &str,
        flow_id: &str,
    ) -> Result<FlowDefinition, CommandBusAdapterError> {
        self.run(
            self.bus
                .get_flow(workspace_id.to_string(), flow_id.to_string()),
        )
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn save_flow(&self, input: FlowDefinition) -> Result<FlowDefinition, CommandBusAdapterError> {
        self.run(self.bus.save_flow(input))
            .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn delete_flow(
        &self,
        workspace_id: &str,
        flow_id: &str,
        expected_revision: i64,
    ) -> Result<(), CommandBusAdapterError> {
        self.run(self.bus.delete_flow(
            workspace_id.to_string(),
            flow_id.to_string(),
            expected_revision,
        ))
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn run_flow(
        &self,
        input: FlowRunInput,
        expected_revision: i64,
    ) -> Result<FlowRun, CommandBusAdapterError> {
        self.run(
            self.bus
                .run_flow_at_revision(input, Some(expected_revision)),
        )
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn cancel_flow_run(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> Result<FlowRun, CommandBusAdapterError> {
        self.run(
            self.bus
                .cancel_flow_run(workspace_id.to_string(), run_id.to_string()),
        )
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn list_flow_runs(
        &self,
        workspace_id: &str,
        flow_id: &str,
    ) -> Result<Vec<FlowRunSummary>, CommandBusAdapterError> {
        self.run(
            self.bus
                .list_flow_runs(workspace_id.to_string(), flow_id.to_string()),
        )
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn list_flow_runs_page(
        &self,
        workspace_id: &str,
        flow_id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<FlowRunPage, CommandBusAdapterError> {
        self.run(self.bus.list_flow_runs_page(
            workspace_id.to_string(),
            flow_id.to_string(),
            limit,
            cursor.map(str::to_owned),
        ))
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn get_flow_run(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> Result<FlowRun, CommandBusAdapterError> {
        self.run(
            self.bus
                .get_flow_run(workspace_id.to_string(), run_id.to_string()),
        )
        .map_err(|e| CommandBusAdapterError::from_flow_error(&e))
    }
    fn list_db_history(
        &self,
        workspace_id: &str,
        limit: i64,
    ) -> Result<Vec<unfour_core::models::DbQueryHistoryEntry>, CommandBusAdapterError> {
        self.run(
            self.bus
                .list_database_query_history(workspace_id.into(), Some(limit)),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_database_app_error("Database history read failed.", &e)
        })
    }
    fn delete_db_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<(), CommandBusAdapterError> {
        self.run(
            self.bus
                .delete_database_connection(workspace_id.into(), connection_id.into()),
        )
        .map(|_| ())
        .map_err(|e| {
            CommandBusAdapterError::from_database_app_error(
                "Database connection deletion failed.",
                &e,
            )
        })
    }
    fn delete_ssh_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<(), CommandBusAdapterError> {
        self.run(
            self.bus
                .delete_ssh_connection(workspace_id.into(), connection_id.into()),
        )
        .map(|_| ())
        .map_err(|e| {
            CommandBusAdapterError::from_ssh_app_error("SSH connection deletion failed.", &e)
        })
    }
    fn test_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<unfour_core::models::SshTestResult, CommandBusAdapterError> {
        #[cfg(not(feature = "ssh-native"))]
        {
            let _ = input;
            Err(CommandBusAdapterError {
                code: "COMMAND_BUS_OPERATION_UNSUPPORTED",
                message: "Native SSH is unavailable.",
                details: serde_json::json!({}),
            })
        }
        #[cfg(feature = "ssh-native")]
        self.run_execution(self.bus.test_ssh_connection(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error("SSH connection test failed.", &e)
            })
    }
    fn get_ssh_host_key(
        &self,
        input: unfour_core::models::SshHostKeyInput,
    ) -> Result<Option<unfour_core::models::SshHostFingerprintInfo>, CommandBusAdapterError> {
        self.run(self.bus.get_ssh_host_fingerprint(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error("SSH host fingerprint read failed.", &e)
            })
    }
    fn execute_read(
        &self,
        command: ReadCommand,
    ) -> Result<ReadCommandResult, CommandBusAdapterError> {
        self.run(self.bus.execute_read(command)).map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus read operation failed.", &e)
        })
    }

    fn execute_saved_api_request(
        &self,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run_execution(self.bus.execute_saved_api_request(request_id, timeout_ms))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API send operation failed.",
                    &e,
                )
            })
    }

    fn execute_saved_api_request_in_workspace(
        &self,
        workspace_id: Option<&str>,
        request_id: &str,
        timeout_ms: Option<u64>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run_execution(self.bus.execute_saved_api_request_in_workspace(
            workspace_id.map(str::to_string),
            request_id,
            timeout_ms,
        ))
        .map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus API send operation failed.", &e)
        })
    }

    fn execute_saved_api_request_with_scripts_in_workspace(
        &self,
        workspace_id: Option<&str>,
        request_id: &str,
        timeout_ms: Option<u64>,
        environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        let execution_id = unfour_core::id::new_id();
        self.run_scripted_api_request(
            &execution_id,
            self.bus
                .execute_saved_api_request_with_scripts_controlled_in_workspace(
                    &execution_id,
                    workspace_id.map(str::to_string),
                    request_id,
                    timeout_ms,
                    environment_id.map(str::to_string),
                ),
        )
    }

    fn execute_saved_api_request_snapshot(
        &self,
        snapshot: &ApiSavedRequest,
        timeout_ms: Option<u64>,
        environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        let execution_id = unfour_core::id::new_id();
        self.run_scripted_api_request(
            &execution_id,
            self.bus.execute_saved_api_request_snapshot_controlled(
                &execution_id,
                snapshot,
                timeout_ms,
                environment_id.map(str::to_string),
            ),
        )
    }

    fn send_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run_execution(self.bus.send_api_request(input))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus API send operation failed.",
                    &e,
                )
            })
    }

    fn send_api_request_in_environment(
        &self,
        input: ApiRequestInput,
        environment_id: Option<&str>,
    ) -> Result<ApiResponse, CommandBusAdapterError> {
        self.run_execution(
            self.bus
                .send_api_request_in_environment(input, environment_id.map(str::to_string)),
        )
        .map_err(|error| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API send operation failed.",
                &error,
            )
        })
    }

    fn save_api_request(
        &self,
        input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.run(self.bus.save_api_request(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API request save operation failed.",
                &e,
            )
        })
    }

    fn update_api_request(
        &self,
        workspace_id: &str,
        request_id: &str,
        input: ApiRequestInput,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.update_saved_request(workspace_id, request_id, input, None)
    }

    fn update_api_request_if_current(
        &self,
        workspace_id: &str,
        request_id: &str,
        input: ApiRequestInput,
        expected_revision: i64,
    ) -> Result<ApiSavedRequest, CommandBusAdapterError> {
        self.update_saved_request(workspace_id, request_id, input, Some(expected_revision))
    }

    fn delete_api_request(
        &self,
        workspace_id: &str,
        request_id: &str,
    ) -> Result<Vec<ApiSavedRequest>, CommandBusAdapterError> {
        self.run(
            self.bus
                .delete_api_request(workspace_id.to_string(), request_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API request delete operation failed.",
                &e,
            )
        })
    }

    fn create_api_collection(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<ApiCollection, CommandBusAdapterError> {
        self.run(
            self.bus
                .api_collection_create(workspace_id.to_string(), name.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API collection create operation failed.",
                &e,
            )
        })
    }

    fn update_api_collection(
        &self,
        workspace_id: &str,
        collection_id: &str,
        name: &str,
    ) -> Result<ApiCollection, CommandBusAdapterError> {
        self.run(self.bus.api_collection_rename(
            workspace_id.to_string(),
            collection_id.to_string(),
            name.to_string(),
        ))
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API collection update operation failed.",
                &e,
            )
        })
    }

    fn delete_api_collection(
        &self,
        workspace_id: &str,
        collection_id: &str,
    ) -> Result<Vec<ApiCollection>, CommandBusAdapterError> {
        self.run(
            self.bus
                .api_collection_delete(workspace_id.to_string(), collection_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API collection delete operation failed.",
                &e,
            )
        })
    }

    fn create_api_environment(
        &self,
        workspace_id: &str,
        name: &str,
    ) -> Result<ApiEnvironment, CommandBusAdapterError> {
        self.run(
            self.bus
                .api_environment_create(workspace_id.to_string(), name.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API environment create operation failed.",
                &e,
            )
        })
    }

    fn update_api_environment(
        &self,
        workspace_id: &str,
        environment_id: &str,
        name: &str,
        variables: Vec<KeyValue>,
    ) -> Result<ApiEnvironment, CommandBusAdapterError> {
        self.run(self.bus.api_environment_update(
            workspace_id.to_string(),
            environment_id.to_string(),
            name.to_string(),
            variables,
        ))
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API environment update operation failed.",
                &e,
            )
        })
    }

    fn delete_api_environment(
        &self,
        workspace_id: &str,
        environment_id: &str,
    ) -> Result<Vec<ApiEnvironment>, CommandBusAdapterError> {
        self.run(
            self.bus
                .api_environment_delete(workspace_id.to_string(), environment_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API environment delete operation failed.",
                &e,
            )
        })
    }

    fn list_workspace_environments(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceEnvironment>, CommandBusAdapterError> {
        self.run(
            self.bus
                .workspace_environments_list(workspace_id.to_string()),
        )
        .map_err(|error| {
            CommandBusAdapterError::from_app_error(
                "The command-bus workspace environment read operation failed.",
                &error,
            )
        })
    }

    fn create_api_environment_variable(
        &self,
        workspace_id: &str,
        environment_id: &str,
        input: WorkspaceVariableInput,
    ) -> Result<WorkspaceEnvironmentVariable, CommandBusAdapterError> {
        self.run(self.bus.workspace_environment_variable_create(
            workspace_id.to_string(),
            environment_id.to_string(),
            input,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API environment variable create operation failed.",
                &error,
            )
        })
    }

    fn update_api_environment_variable(
        &self,
        workspace_id: &str,
        environment_id: &str,
        variable_id: &str,
        input: WorkspaceVariableInput,
    ) -> Result<WorkspaceEnvironmentVariable, CommandBusAdapterError> {
        self.run(self.bus.workspace_environment_variable_update(
            workspace_id.to_string(),
            environment_id.to_string(),
            variable_id.to_string(),
            input,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API environment variable update operation failed.",
                &error,
            )
        })
    }

    fn delete_api_environment_variable(
        &self,
        workspace_id: &str,
        environment_id: &str,
        variable_id: &str,
    ) -> Result<Vec<WorkspaceEnvironmentVariable>, CommandBusAdapterError> {
        self.run(self.bus.workspace_environment_variable_delete(
            workspace_id.to_string(),
            environment_id.to_string(),
            variable_id.to_string(),
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_app_error(
                "The command-bus API environment variable delete operation failed.",
                &error,
            )
        })
    }

    fn list_db_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<DatabaseConnection>, CommandBusAdapterError> {
        self.run(self.bus.list_database_connections(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_database_app_error(
                    "The command-bus database list operation failed.",
                    &e,
                )
            })
    }

    fn list_workspace_variables(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        self.run(self.bus.workspace_variables_list(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus workspace variable list operation failed.",
                    &e,
                )
            })
    }

    fn replace_workspace_variables(
        &self,
        workspace_id: &str,
        variables: Vec<WorkspaceVariableInput>,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        self.run(
            self.bus
                .workspace_variables_replace(workspace_id.to_string(), variables),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus workspace variable replacement failed.",
                &e,
            )
        })
    }

    fn create_workspace_variable(
        &self,
        workspace_id: &str,
        input: WorkspaceVariableInput,
    ) -> Result<WorkspaceVariable, CommandBusAdapterError> {
        self.run(
            self.bus
                .workspace_variable_create(workspace_id.to_string(), input),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus workspace variable create operation failed.",
                &e,
            )
        })
    }

    fn update_workspace_variable(
        &self,
        workspace_id: &str,
        variable_id: &str,
        input: WorkspaceVariableInput,
    ) -> Result<WorkspaceVariable, CommandBusAdapterError> {
        self.run(self.bus.workspace_variable_update(
            workspace_id.to_string(),
            variable_id.to_string(),
            input,
        ))
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus workspace variable update operation failed.",
                &e,
            )
        })
    }

    fn delete_workspace_variable(
        &self,
        workspace_id: &str,
        variable_id: &str,
    ) -> Result<Vec<WorkspaceVariable>, CommandBusAdapterError> {
        self.run(
            self.bus
                .workspace_variable_delete(workspace_id.to_string(), variable_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus workspace variable delete operation failed.",
                &e,
            )
        })
    }

    fn save_db_connection(
        &self,
        input: DatabaseConnectionInput,
    ) -> Result<DatabaseConnection, CommandBusAdapterError> {
        self.run(self.bus.save_database_connection(input))
            .map_err(|e| {
                CommandBusAdapterError::from_database_app_error(
                    "The command-bus database connection save failed.",
                    &e,
                )
            })
    }

    fn create_credential(
        &self,
        input: CredentialCreateInput,
    ) -> Result<CredentialMetadata, CommandBusAdapterError> {
        self.run(self.bus.create_credential(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus credential create operation failed.",
                &e,
            )
        })
    }

    fn get_db_schema(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        self.get_db_schema_for_catalog(workspace_id, connection_id, None)
    }

    fn get_db_schema_for_catalog(
        &self,
        workspace_id: &str,
        connection_id: &str,
        catalog: Option<&str>,
    ) -> Result<DatabaseSchema, CommandBusAdapterError> {
        self.run_execution(self.bus.database_schema(
            workspace_id.to_string(),
            connection_id.to_string(),
            catalog.map(str::to_string),
        ))
        .map_err(|e| {
            CommandBusAdapterError::from_database_app_error(
                "The command-bus database schema operation failed.",
                &e,
            )
        })
    }

    fn list_db_tables(
        &self,
        workspace_id: &str,
        connection_id: &str,
        catalog: Option<&str>,
        schema: Option<&str>,
        limit: u32,
    ) -> Result<DatabaseTableList, CommandBusAdapterError> {
        self.run_execution(self.bus.database_list_tables(
            workspace_id.to_string(),
            connection_id.to_string(),
            catalog.map(str::to_string),
            schema.map(str::to_string),
            limit,
        ))
        .map_err(|error| {
            CommandBusAdapterError::from_database_app_error(
                "The table listing operation failed.",
                &error,
            )
        })
    }

    fn execute_db_query(
        &self,
        input: DatabaseQueryInput,
    ) -> Result<DatabaseQueryResult, CommandBusAdapterError> {
        self.run_execution(self.bus.execute_database_query(input))
            .map_err(|e| {
                CommandBusAdapterError::from_database_app_error(
                    "The command-bus database query operation failed.",
                    &e,
                )
            })
    }

    fn test_db_connection(
        &self,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<DatabaseTestResult, CommandBusAdapterError> {
        self.run_execution(
            self.bus
                .test_database_connection(workspace_id.to_string(), connection_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_database_app_error(
                "The command-bus database connection test failed.",
                &e,
            )
        })
    }

    fn system_health(&self) -> Result<SystemHealth, CommandBusAdapterError> {
        self.run(self.bus.system_health()).map_err(|e| {
            CommandBusAdapterError::from_app_error("The command-bus system health read failed.", &e)
        })
    }

    fn run_ssh_diagnostic(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.run_execution(self.bus.run_ssh_diagnostic(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error(
                    "The command-bus SSH diagnostic failed.",
                    &e,
                )
            })
    }

    fn list_ssh_connections(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SshConnection>, CommandBusAdapterError> {
        self.run(self.bus.list_ssh_connections(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus SSH connection list operation failed.",
                    &e,
                )
            })
    }

    fn save_ssh_connection(
        &self,
        input: SshConnectionInput,
    ) -> Result<SshConnection, CommandBusAdapterError> {
        self.run(self.bus.save_ssh_connection(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH connection save operation failed.",
                &e,
            )
        })
    }

    fn list_ssh_command_history(
        &self,
        query: SshCommandHistoryQuery,
    ) -> Result<Vec<SshCommandHistoryEntry>, CommandBusAdapterError> {
        self.run(self.bus.list_ssh_command_history(query))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus SSH command history list operation failed.",
                    &e,
                )
            })
    }

    fn write_ssh_file(
        &self,
        input: SshFileWriteInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.run_execution(self.bus.write_ssh_file(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error(
                    "SSH file operation failed; inspect the target before retrying.",
                    &e,
                )
            })
    }

    fn patch_ssh_file(
        &self,
        input: SshFilePatchInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.run_execution(self.bus.patch_ssh_file(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error(
                    "SSH file operation failed; inspect the target before retrying.",
                    &e,
                )
            })
    }

    fn run_ssh_command(
        &self,
        input: SshDiagnosticInput,
    ) -> Result<SshDiagnosticResult, CommandBusAdapterError> {
        self.run_execution(self.bus.run_ssh_command(input))
            .map_err(|e| {
                CommandBusAdapterError::from_ssh_app_error(
                    "The command-bus SSH command failed.",
                    &e,
                )
            })
    }

    fn list_ssh_tasks(&self, workspace_id: &str) -> Result<Vec<SshTask>, CommandBusAdapterError> {
        self.run(self.bus.list_ssh_tasks(workspace_id.to_string()))
            .map_err(|e| {
                CommandBusAdapterError::from_app_error(
                    "The command-bus SSH task list operation failed.",
                    &e,
                )
            })
    }

    fn reorder_ssh_tasks(
        &self,
        input: SshTasksReorderInput,
    ) -> Result<Vec<SshTask>, CommandBusAdapterError> {
        self.run(self.bus.reorder_ssh_tasks(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task reorder operation failed.",
                &e,
            )
        })
    }

    fn get_ssh_task(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        self.run(
            self.bus
                .get_ssh_task(workspace_id.to_string(), task_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task detail operation failed.",
                &e,
            )
        })
    }

    fn save_ssh_task(
        &self,
        input: SshTaskSaveInput,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        self.run(self.bus.save_ssh_task(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task save operation failed.",
                &e,
            )
        })
    }

    fn duplicate_ssh_task(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<SshTaskDetail, CommandBusAdapterError> {
        self.run(
            self.bus
                .duplicate_ssh_task(workspace_id.to_string(), task_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task duplicate operation failed.",
                &e,
            )
        })
    }

    fn delete_ssh_task(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<(), CommandBusAdapterError> {
        self.run(
            self.bus
                .delete_ssh_task(workspace_id.to_string(), task_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task delete operation failed.",
                &e,
            )
        })
    }

    fn run_ssh_task(&self, input: SshTaskRunInput) -> Result<SshTaskRun, CommandBusAdapterError> {
        self.run(self.bus.run_ssh_task(input)).map_err(|e| {
            CommandBusAdapterError::from_ssh_app_error(
                "The command-bus SSH task execution failed.",
                &e,
            )
        })
    }

    fn cancel_ssh_task_run(
        &self,
        input: SshTaskCancelInput,
    ) -> Result<SshTaskRun, CommandBusAdapterError> {
        self.run(self.bus.cancel_ssh_task_run(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task cancellation failed.",
                &e,
            )
        })
    }

    fn list_ssh_task_runs(
        &self,
        workspace_id: &str,
        task_id: &str,
    ) -> Result<Vec<SshTaskRun>, CommandBusAdapterError> {
        self.run(
            self.bus
                .list_ssh_task_runs(workspace_id.to_string(), task_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task run list operation failed.",
                &e,
            )
        })
    }

    fn read_ssh_task_run_log(
        &self,
        workspace_id: &str,
        run_id: &str,
    ) -> Result<String, CommandBusAdapterError> {
        self.run(
            self.bus
                .read_ssh_task_run_log(workspace_id.to_string(), run_id.to_string()),
        )
        .map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task log read operation failed.",
                &e,
            )
        })
    }

    fn clear_ssh_task_runs(
        &self,
        input: SshTaskCleanupInput,
    ) -> Result<SshTaskCleanupResult, CommandBusAdapterError> {
        self.run(self.bus.clear_ssh_task_runs(input)).map_err(|e| {
            CommandBusAdapterError::from_app_error(
                "The command-bus SSH task run cleanup failed.",
                &e,
            )
        })
    }
}
