use bot::prelude::*;

use super::MyState;

pub struct Echo;

#[async_trait]
impl ExecutableCommand<MyState> for Echo {
    async fn execute<'a>(
        &self,
        command: &'a str,
        message: irc::Message<'a>,
        _: &BotState<MyState>,
    ) -> ExecutionOutcome {
        if command.is_empty() {
            info!("nothing to echo!");
            ExecutionOutcome::SilentSuccess
        } else {
            ExecutionOutcome::success(
                message.first_arg_as_channel_name().unwrap().to_string(),
                command.to_string(),
            )
        }
    }

    fn help(&self) -> String {
        "echo <message> -- echoes message back".to_string()
    }

    fn cooldown(&self) -> CommandCooldown {
        CommandCooldown {
            command: Some(Duration::from_secs(1)),
            user: None,
        }
    }

    fn level(&self) -> PermissionLevel {
        PermissionLevel::Admin
    }
}
