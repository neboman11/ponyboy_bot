use std::env;
use std::path::Path;

use regex::Regex;
use serenity::all::UserId;
use serenity::async_trait;
use serenity::builder::{CreateAttachment, CreateMessage};
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

mod keyword_action;

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
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.id != ctx.cache.current_user().id {
            for keyword_action in &self.keyword_actions {
                let mut message_matches_action = false;

                // Check if a specific user is mentioned in the message
                let triggers = keyword_action.triggers.as_ref().unwrap();
                if triggers.contains(&"mention".to_string()) {
                    let mentioned_user = keyword_action.mentioned_user.as_ref().unwrap();
                    let user_id = UserId::new(*mentioned_user);
                    if msg.mentions_user_id(user_id) {
                        message_matches_action = true;
                    }
                }

                // Check if a keyword is used in the message
                let regex_keyword_group = keyword_action
                    .keywords
                    .as_ref()
                    .unwrap()
                    .join(r"( |[\?\.',]|$)|(^| )");
                let re = Regex::new(format!("(^| ){regex_keyword_group}( |[\\?\\.',]|$)").as_str())
                    .unwrap();
                if re.is_match(msg.content.as_str()) {
                    message_matches_action = true;
                }
                if message_matches_action {
                    let actions = keyword_action.actions.as_ref().unwrap();
                    for action in actions {
                        match action.as_str() {
                            "emote" => {
                                let emotes = keyword_action.emotes.as_ref().unwrap();
                                for emote in emotes {
                                    if emote.chars().count() > 1 {
                                        let emoji_id = serenity::all::EmojiId::new(
                                            emote.parse::<u64>().unwrap(),
                                        );
                                        let emoji = msg
                                            .guild_id
                                            .unwrap()
                                            .emoji(&ctx, emoji_id)
                                            .await
                                            .unwrap();
                                        if let Err(why) = msg.react(&ctx, emoji).await {
                                            println!("Error sending message: {why:?}");
                                        }
                                    } else {
                                        if let Err(why) =
                                            msg.react(&ctx, emote.chars().next().unwrap()).await
                                        {
                                            println!("Error sending message: {why:?}");
                                        }
                                    }
                                }
                                println!(
                                    "{}: {} - {:#?}",
                                    keyword_action.name.as_ref().unwrap(),
                                    action,
                                    emotes
                                );
                            }
                            "message" => {
                                let message = keyword_action.message.as_ref().unwrap();
                                if let Err(why) = msg.channel_id.say(&ctx.http, message).await {
                                    println!("Error sending message: {why:?}");
                                }
                                println!(
                                    "{}: {} - {}",
                                    keyword_action.name.as_ref().unwrap(),
                                    action,
                                    message
                                );
                            }
                            "file_embed" => {
                                let file = keyword_action.file.as_ref().unwrap();
                                let paths = [CreateAttachment::path(
                                    Path::new(&self.file_base_dir).join(file),
                                )
                                .await
                                .unwrap()];

                                let builder = CreateMessage::new();
                                let message: String;
                                match keyword_action.message.as_ref() {
                                    Some(config_message) => {
                                        message = config_message.clone();
                                    }
                                    None => {
                                        message = file.clone();
                                    }
                                }
                                if let Err(why) =
                                    msg.channel_id.send_files(&ctx.http, paths, builder).await
                                {
                                    println!("Error sending message: {why:?}");
                                }
                                println!(
                                    "{}: {} - {}",
                                    keyword_action.name.as_ref().unwrap(),
                                    action,
                                    message
                                );
                            }
                            _ => {
                                println!("Unknown action: {}", action);
                            }
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
    let file_base_dir =
        env::var("FILE_BASE_DIR").expect("Expected file base dir to be set in the environment");

    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&token, intents)
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
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
