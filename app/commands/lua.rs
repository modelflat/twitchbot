use modelflat_bot::core::prelude::*;

use super::MyState;

pub struct Lua;

#[async_trait]
impl ExecutableCommand<MyState> for Lua {
    async fn execute<'a>(
        &self,
        command: &'a str,
        message: irc::Message<'a>,
        _: &ShareableBotState<MyState>,
        _: &ReadonlyState<MyState>,
    ) -> ExecutionOutcome {
        use modelflat_bot::core::lua::run_untrusted_lua_code;

        if !command.is_empty() {
            let user = message
                .tag_value("display-name")
                .unwrap_or("<no-display-name>");

            info!("{} is executing Lua: {}", user, command);

            let result = run_untrusted_lua_code(command.to_string());

            ExecutionOutcome::success(
                message.first_arg_as_channel_name().unwrap().to_string(),
                match result {
                    Ok(result) => format!("@{}, result = {}", user, result),
                    Err(err) => format!("@{}, error! {}", user, err),
                },
            )
        } else {
            info!("lua: not enough arguments");
            ExecutionOutcome::Error("lua: not enough arguments".to_string())
        }
    }

    fn help(&self) -> String {
        "lua <code> -- executes your code in a Lua sandbox. \
        limits: 640kb of memory, 1000 instructions FeelsGoodMan".to_string()
    }

    fn cooldown(&self) -> (Option<Duration>, Option<Duration>) {
        (Some(Duration::from_secs(2)), Some(Duration::from_secs(10)))
    }

    fn level(&self) -> PermissionLevel {
        PermissionLevel::User
    }
}