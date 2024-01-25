use std::env;
use std::path::Path;

use rand::seq::SliceRandom;
use regex::Regex;
use serenity::all::{ReactionType, UserId};
use serenity::async_trait;
use serenity::builder::{CreateAttachment, CreateMessage, GetMessages};
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

mod ai;
mod api;
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
    async fn message(&self, ctx: Context, incoming_message: Message) {
        if incoming_message.author.id != ctx.cache.current_user().id {
            if incoming_message.mentions_user_id(ctx.cache.current_user().id) {
                let message_history_builder =
                    GetMessages::new().before(incoming_message.id).limit(10);
                let mut message_history = incoming_message
                    .channel_id
                    .messages(&ctx.http, message_history_builder)
                    .await
                    .unwrap();
                message_history.reverse();
                let message_history_string = convert_message_list_to_history_string(
                    ctx.cache.current_user().id.into(),
                    message_history,
                );
                let trimmed_message = incoming_message.content.replace(
                    format!("<@{}>", ctx.cache.current_user().id).as_str(),
                    "ponyboy",
                );
                if let Ok(generated_message) = ai::generate_ai_bot_response(
                    incoming_message.author.name.clone(),
                    trimmed_message,
                    message_history_string,
                )
                .await
                {
                    if let Err(why) = incoming_message
                        .channel_id
                        .say(&ctx.http, &generated_message)
                        .await
                    {
                        println!("Error sending message: {why:?}");
                    }
                    println!(
                        "{}: {} - {}",
                        "generated_message", "message", generated_message
                    );
                }
            }
            for keyword_action in &self.keyword_actions {
                let mut message_matches_action = false;

                // Check if a specific user is mentioned in the message
                let triggers = keyword_action.triggers.as_ref().unwrap();
                if triggers.contains(&"mention".to_string()) {
                    let mentioned_user = keyword_action.mentioned_user.as_ref().unwrap();
                    let user_id = UserId::new(*mentioned_user);
                    if incoming_message.mentions_user_id(user_id) {
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
                if re.is_match(incoming_message.content.as_str()) {
                    message_matches_action = true;
                }
                if message_matches_action {
                    let random_action = keyword_action
                        .actions
                        .as_ref()
                        .unwrap()
                        .choose(&mut rand::thread_rng())
                        .unwrap();
                    if let Some(emotes) = random_action.emotes.as_ref() {
                        for emote in emotes {
                            if let Ok(emote_id) = emote.parse::<u64>() {
                                let emoji_id = serenity::all::EmojiId::new(emote_id);
                                let emoji = incoming_message
                                    .guild_id
                                    .unwrap()
                                    .emoji(&ctx, emoji_id)
                                    .await
                                    .unwrap();
                                if let Err(why) = incoming_message.react(&ctx, emoji).await {
                                    println!("Error sending message: {why:?}");
                                }
                            } else {
                                if let Err(why) = incoming_message
                                    .react(&ctx, ReactionType::Unicode(emote.clone()))
                                    .await
                                {
                                    println!("Error sending message: {why:?}");
                                }
                            }
                        }
                        println!(
                            "{}: {} - {:#?}",
                            keyword_action.name.as_ref().unwrap(),
                            "emote",
                            emotes
                        );
                    }
                    let mut sending_embed_message = false;
                    if let Some(file) = random_action.file.as_ref() {
                        sending_embed_message = true;
                        let paths =
                            [
                                CreateAttachment::path(Path::new(&self.file_base_dir).join(file))
                                    .await
                                    .unwrap(),
                            ];

                        let builder;
                        let message: String;
                        let mut add_message = false;
                        match random_action.message.as_ref() {
                            Some(config_message) => {
                                message = config_message.clone();
                                add_message = true;
                            }
                            None => {
                                message = file.clone();
                            }
                        }
                        if add_message {
                            builder = CreateMessage::new().content(&message);
                        } else {
                            builder = CreateMessage::new();
                        }
                        if let Err(why) = incoming_message
                            .channel_id
                            .send_files(&ctx.http, paths, builder)
                            .await
                        {
                            println!("Error sending message: {why:?}");
                        }
                        println!(
                            "{}: {} - {}",
                            keyword_action.name.as_ref().unwrap(),
                            "file_embed",
                            message
                        );
                    }
                    if let Some(message) = random_action.message.as_ref() {
                        if !sending_embed_message {
                            if let Err(why) =
                                incoming_message.channel_id.say(&ctx.http, message).await
                            {
                                println!("Error sending message: {why:?}");
                            }
                            println!(
                                "{}: {} - {}",
                                keyword_action.name.as_ref().unwrap(),
                                "message",
                                message
                            );
                        }
                    }
                    if let Some(message) = random_action.mention.as_ref() {
                        let mentioned_user = incoming_message.author.mention();
                        let formatted_messaage =
                            message.replace("@mention", format!("{}", mentioned_user).as_str());
                        if let Err(why) = incoming_message
                            .channel_id
                            .say(&ctx.http, formatted_messaage)
                            .await
                        {
                            println!("Error sending message: {why:?}");
                        }
                        println!(
                            "{}: {} - {}",
                            keyword_action.name.as_ref().unwrap(),
                            "message",
                            message
                        );
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

fn convert_message_list_to_history_string(bot_id: u64, message_list: Vec<Message>) -> String {
    let mut message_string_list = Vec::new();

    for message in message_list {
        println!("{}", message.content);
        if message.content != "" {
            message_string_list.push(format!(
                "{}: {}",
                message.author.name,
                message
                    .content
                    .replace(format!("<@{}>", bot_id).as_str(), "ponyboy",)
            ));
        }
    }

    return message_string_list.join("\n");
}
