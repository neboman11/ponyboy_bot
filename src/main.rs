use std::env;

use regex::Regex;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

mod keyword_action;

struct Handler {
    pub(crate) keyword_actions: Vec<keyword_action::KeywordAction>,
}

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event - so that whenever a new message is received - the
    // closure (or function) passed will be called.
    //
    // Event handlers are dispatched through a threadpool, and so multiple events can be dispatched
    // simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        for keyword_action in &self.keyword_actions {
            let regex_keyword_group = keyword_action
                .keywords
                .as_ref()
                .unwrap()
                .join(r"( |[\?\.',]|$)|(^| )");
            let re =
                Regex::new(format!("(^| ){regex_keyword_group}( |[\\?\\.',]|$)").as_str()).unwrap();
            if re.is_match(msg.content.as_str()) {
                let actions = keyword_action.actions.as_ref().unwrap();
                for action in actions {
                    match action.as_str() {
                        "emote" => {
                            let emotes = keyword_action.emotes.as_ref().unwrap();
                            for emote in emotes {
                                if let Err(why) = msg.channel_id.say(&ctx.http, emote).await {
                                    println!("Error sending message: {why:?}");
                                }
                            }
                            println!("{}: {:#?}", keyword_action.name.as_ref().unwrap(), emotes);
                        }
                        "message" => {
                            let message = keyword_action.message.as_ref().unwrap();
                            if let Err(why) = msg.channel_id.say(&ctx.http, message).await {
                                println!("Error sending message: {why:?}");
                            }
                            println!("{}: {}", keyword_action.name.as_ref().unwrap(), message);
                        }
                        "file_embed" => {
                            let message = keyword_action.message.as_ref().unwrap();
                            if let Err(why) = msg.channel_id.say(&ctx.http, message).await {
                                println!("Error sending message: {why:?}");
                            }
                            println!("{}: {}", keyword_action.name.as_ref().unwrap(), message);
                        }
                        _ => {
                            println!("Unknown action: {}", action);
                        }
                    }
                }
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
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let keyword_actions = keyword_action::load_keyword_actions();

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            keyword_actions: keyword_actions,
        })
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
