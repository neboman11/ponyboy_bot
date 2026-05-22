use std::env;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

mod ai;
mod api;
mod keyword_action;
mod message_processing;

struct Handler {
    keyword_actions: Vec<keyword_action::KeywordAction>,
    file_base_dir: String,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, incoming_message: Message) {
        if incoming_message.author.id != ctx.cache.current_user().id {
            if incoming_message.mentions_user_id(ctx.cache.current_user().id) {
                message_processing::send_llm_generated_message(&ctx, incoming_message).await;
            } else {
                message_processing::process_keyword_actions(
                    &ctx,
                    incoming_message,
                    &self.keyword_actions,
                    &self.file_base_dir,
                )
                .await;
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    let discord_token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let keyword_actions = keyword_action::load_keyword_actions();
    let file_base_dir =
        env::var("FILE_BASE_DIR").expect("Expected file base dir to be set in the environment");

    let mut discord_client = Client::builder(&discord_token, intents)
        .event_handler(Handler {
            file_base_dir,
            keyword_actions,
        })
        .await
        .expect("Err creating client");

    let discord_bot = async move {
        if let Err(why) = discord_client.start().await {
            println!("Client error: {why:?}");
        }
    };

    let rest_server = api::start_api_server(discord_token);

    futures::join!(rest_server, discord_bot);
}
