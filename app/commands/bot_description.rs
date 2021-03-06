use bot::prelude::*;

use super::MyState;

pub struct BotDescription;

#[async_trait]
impl ExecutableCommand<MyState> for BotDescription {
    async fn execute<'a>(&self, _: &'a str, message: irc::Message<'a>, _: &BotState<MyState>) -> ExecutionOutcome {
        ExecutionOutcome::success(
            message.first_arg_as_channel_name().unwrap().to_string(),
            "\
            FeelsDankMan I'm a bot by modelflat. \
            Prefix: '>>'. \
            Language: Rust (nightly). \
            See (>> help) for commands. \
            Source code at github: modelflat/twitchbot"
                .to_string(),
        )
    }

    fn help(&self) -> String {
        "bot -- describes bot".to_string()
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
