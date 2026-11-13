use super::{CommandBusAdapterError, LocalCommandBusAdapter};
use unfour_command_bus::RedisCheckResult;

/// Runtime-only operands, interpreted as exact bytes and never logged.
pub struct RedisCheck<'a> {
    pub workspace: &'a str,
    pub connection: &'a str,
    pub key: &'a [u8],
    pub field: Option<&'a [u8]>,
    pub equals: Option<&'a [u8]>,
}

impl LocalCommandBusAdapter {
    pub(super) fn run_redis_check(
        &self,
        input: RedisCheck<'_>,
    ) -> Result<RedisCheckResult, CommandBusAdapterError> {
        let read = self.bus.check_redis_key(
            input.workspace,
            input.connection,
            input.key,
            input.field,
            input.equals,
        );
        self.run_execution(read)
            .map_err(|error| CommandBusAdapterError {
                code: error.code(),
                message: "The Redis key check failed.",
                details: serde_json::json!({}),
            })
    }
}
