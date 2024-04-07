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
    pub(crate) keyword_actions: Vec<keyword_action::KeywordAction>,
    pub(crate) file_base_dir: String,
}

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event - so that whenever a new message is received - the
    // closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a threadpool, and so multiple events can be dispatched
    // simultaneously.
    async fn message(&self, ctx: Context, incoming_message: Message) {
        if incoming_message.author.id != ctx.cache.current_user().id {
            // Check if message mentions ponyboy
            // This indicates user is requesting ponyboy to generate a response
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

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let discord_token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let keyword_actions = keyword_action::load_keyword_actions();
    let file_base_dir =
        env::var("FILE_BASE_DIR").expect("Expected file base dir to be set in the environment");

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut discord_client = Client::builder(&discord_token, intents)
        .event_handler(Handler {
            file_base_dir: file_base_dir,
            keyword_actions: keyword_actions,
        })
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    let discord_bot = async move {
        if let Err(why) = discord_client.start().await {
            println!("Client error: {why:?}");
        }
    };

    let rest_server = api::start_api_server(discord_token);

    // Running both the REST server and the Discord bot concurrently
    futures::join!(rest_server, discord_bot);
}
