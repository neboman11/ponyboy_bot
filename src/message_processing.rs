use std::path::Path;

use rand::seq::SliceRandom;
use regex::Regex;
use serenity::all::{GetMessages, ReactionType, UserId};
use serenity::builder::{CreateAttachment, CreateMessage};
use serenity::model::channel::Message;
use serenity::prelude::*;

use crate::{ai, keyword_action};

pub(crate) async fn send_llm_generated_message(ctx: &Context, incoming_message: Message) {
    let message_history_builder = GetMessages::new().before(incoming_message.id).limit(10);
    let mut message_history = incoming_message
        .channel_id
        .messages(&ctx.http, message_history_builder)
        .await
        .unwrap();
    message_history.reverse();
    let message_history_string =
        convert_message_list_to_history(ctx.cache.current_user().id.into(), message_history);
    let trimmed_message = incoming_message.content.replace(
        format!("<@{}>", ctx.cache.current_user().id).as_str(),
        "ponyboy",
    );
    match ai::generate_ai_bot_response(
        incoming_message.author.name.clone(),
        trimmed_message,
        message_history_string,
    )
    .await
    {
        Ok(generated_message) => {
            // If the generated message is too long, break it into multiple messages
            if generated_message.chars().count() > 2000 {
                let mut current_chunk: String = String::new();
                let mut message_chars = generated_message.chars();
                while let Some(message_char) = message_chars.next() {
                    current_chunk.push(message_char);
                    if current_chunk.chars().count() > 1999 {
                        if let Err(why) = incoming_message
                            .channel_id
                            .say(&ctx.http, &current_chunk)
                            .await
                        {
                            println!("Error sending message: {why:?}");
                        }
                        current_chunk.clear();
                    }
                }
                if current_chunk.len() > 0 {
                    if let Err(why) = incoming_message
                        .channel_id
                        .say(&ctx.http, &current_chunk)
                        .await
                    {
                        println!("Error sending message: {why:?}");
                    }
                }
            } else {
                if let Err(why) = incoming_message
                    .channel_id
                    .say(&ctx.http, &generated_message)
                    .await
                {
                    println!("Error sending message: {why:?}");
                }
            }
            println!("{}: {}", "generated_message", "message");
        }
        Err(error) => {
            println!("Unable to generate message response: {}", error)
        }
    }
}

pub(crate) async fn process_keyword_actions(
    ctx: &Context,
    incoming_message: Message,
    keyword_actions: &Vec<keyword_action::KeywordAction>,
    file_base_dir: &String,
) {
    for keyword_action in keyword_actions {
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
        let re =
            Regex::new(format!("(^| ){regex_keyword_group}( |[\\?\\.',]|$)").as_str()).unwrap();
        if re.is_match(incoming_message.content.as_str()) {
            message_matches_action = true;
        }
        if message_matches_action {
            // Select a random action from the list of actions associated with the trigger. If only one action is specified, it is selected
            let random_action = keyword_action
                .actions
                .as_ref()
                .unwrap()
                .choose(&mut rand::thread_rng())
                .unwrap();
            if let Some(emotes) = random_action.emotes.as_ref() {
                process_emotes_action(
                    &ctx,
                    &incoming_message,
                    &emotes,
                    keyword_action.name.as_ref().unwrap(),
                )
                .await
            }
            let mut sending_embed_message = false;
            if let Some(file) = random_action.file.as_ref() {
                sending_embed_message = true;
                process_file_action(
                    &ctx,
                    &incoming_message,
                    &random_action.message,
                    &file,
                    keyword_action.name.as_ref().unwrap(),
                    &file_base_dir,
                )
                .await
            }
            if let Some(message) = random_action.message.as_ref() {
                if !sending_embed_message {
                    process_message_action(
                        &ctx,
                        &incoming_message,
                        message,
                        keyword_action.name.as_ref().unwrap(),
                    )
                    .await
                }
            }
            if let Some(message) = random_action.mention.as_ref() {
                process_mention_action(
                    ctx,
                    &incoming_message,
                    message,
                    keyword_action.name.as_ref().unwrap(),
                )
                .await
            }
        }
    }
}

fn convert_message_list_to_history(
    bot_id: u64,
    message_list: Vec<Message>,
) -> Vec<(String, String, String)> {
    let mut message_string_list = Vec::new();

    for message in message_list {
        if message.content != "" {
            message_string_list.push((
                message.timestamp.to_rfc3339().unwrap(),
                message.author.name,
                format!(
                    "{}",
                    message
                        .content
                        .replace(format!("<@{}>", bot_id).as_str(), "ponyboy",)
                ),
            ));
        }
    }

    return message_string_list;
}

async fn process_emotes_action(
    ctx: &Context,
    incoming_message: &Message,
    emotes: &Vec<String>,
    action_name: &String,
) {
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
    println!("{}: {} - {:#?}", action_name, "emote", emotes);
}

async fn process_file_action(
    ctx: &Context,
    incoming_message: &Message,
    action_message: &Option<String>,
    file: &String,
    action_name: &String,
    file_base_dir: &String,
) {
    let paths = [
        CreateAttachment::path(Path::new(file_base_dir).join("file_embeds").join(file))
            .await
            .unwrap(),
    ];

    let builder;
    let message: String;
    let mut add_message = false;
    match action_message {
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
    println!("{}: {} - {}", action_name, "file_embed", message);
}

async fn process_message_action(
    ctx: &Context,
    incoming_message: &Message,
    message: &String,
    action_name: &String,
) {
    if let Err(why) = incoming_message.channel_id.say(&ctx.http, message).await {
        println!("Error sending message: {why:?}");
    }
    println!("{}: {} - {}", action_name, "message", message);
}

async fn process_mention_action(
    ctx: &Context,
    incoming_message: &Message,
    message: &String,
    action_name: &String,
) {
    let mentioned_user = incoming_message.author.mention();
    let formatted_messaage = message.replace("@mention", format!("{}", mentioned_user).as_str());
    if let Err(why) = incoming_message
        .channel_id
        .say(&ctx.http, formatted_messaage)
        .await
    {
        println!("Error sending message: {why:?}");
    }
    println!("{}: {} - {}", action_name, "message", message);
}
