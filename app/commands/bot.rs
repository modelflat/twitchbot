use modelflat_bot::prelude::*;

use super::MyState;

pub struct Bot;

#[async_trait]
impl ExecutableCommand<MyState> for Bot {
    async fn execute<'a>(
        &self,
        _: &'a str,
        message: irc::Message<'a>,
        _: &ShareableBotState<MyState>,
        _: &ReadonlyState<MyState>,
    ) -> ExecutionOutcome {
        ExecutionOutcome::success(
            message.first_arg_as_channel_name().unwrap().to_string(),
            "\
            FeelsDankMan I'm a bot by @modelflat. \
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

    fn cooldown(&self) -> (Option<Duration>, Option<Duration>) {
        (Some(Duration::from_secs(5)), None)
    }

    fn level(&self) -> PermissionLevel {
        PermissionLevel::User
    }
}
