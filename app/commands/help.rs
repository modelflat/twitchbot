use modelflat_bot::prelude::*;

use super::MyState;

pub struct Help;

#[async_trait]
impl ExecutableCommand<MyState> for Help {
    async fn execute<'a>(
        &self,
        command: &'a str,
        message: irc::Message<'a>,
        state: &BotState<MyState>,
    ) -> ExecutionOutcome {
        ExecutionOutcome::success(
            message.first_arg_as_channel_name().unwrap().to_string(),
            if command.is_empty() {
                format!("commands: {}", {
                    let mut cmds: Vec<String> = state.commands.keys().map(|k| k.to_owned()).collect();
                    cmds.sort_unstable();
                    cmds.join(", ")
                })
            } else {
                match command.split(' ').next() {
                    Some(command_name) => match state.commands.get(command_name) {
                        Some(command) => format!("help: {}", command.help()),
                        None => format!("help: no such command: '{}'", command_name),
                    },
                    None => unreachable!(),
                }
            },
        )
    }

    fn help(&self) -> String {
        "help -- describes bot commands // help <command> -- describes command".to_string()
    }

    fn cooldown(&self) -> CommandCooldown {
        CommandCooldown {
            command: Some(Duration::from_secs(5)),
            user: None,
        }
    }

    fn level(&self) -> PermissionLevel {
        PermissionLevel::User
    }
}
