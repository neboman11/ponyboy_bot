use std::path::Path;

use rand::seq::SliceRandom;
use regex::Regex;
use serenity::all::{GetMessages, ReactionType, UserId};
use serenity::builder::{CreateAttachment, CreateMessage};
use serenity::model::channel::Message;
use serenity::prelude::*;

use crate::{ai, keyword_action};

pub(crate) async fn send_llm_generated_message(ctx: &Context, incoming_message: Message) {
    let bot_user = ctx.http.get_current_user().await.unwrap();
    let message_list_builder = GetMessages::new().before(incoming_message.id).limit(10);
    let mut message_list = incoming_message
        .channel_id
        .messages(&ctx.http, message_list_builder)
        .await
        .unwrap();
    message_list.reverse();
    let message_history = convert_message_list_to_history(bot_user.id.into(), message_list);
    let trimmed_message = incoming_message
        .content
        .replace(&format!("<@{}>", bot_user.id), &bot_user.name);

    match ai::generate_ai_bot_response(
        bot_user.name.clone(),
        incoming_message.author.name.clone(),
        trimmed_message,
        message_history,
    )
    .await
    {
        Ok(generated_message) => {
            let chars: Vec<char> = generated_message.chars().collect();
            for chunk in chars.chunks(2000) {
                let chunk_str: String = chunk.iter().collect();
                if let Err(why) = incoming_message.channel_id.say(&ctx.http, &chunk_str).await {
                    println!("Error sending message: {why:?}");
                }
            }
            println!("generated_message: {}", generated_message);
        }
        Err(error) => {
            println!("Unable to generate message response: {}", error);
            if let Err(why) = incoming_message.channel_id.say(&ctx.http, "😴").await {
                println!("Error sending message: {why:?}");
            }
        }
    }
}

pub(crate) async fn process_keyword_actions(
    ctx: &Context,
    incoming_message: Message,
    keyword_actions: &[keyword_action::KeywordAction],
    file_base_dir: &str,
) {
    for keyword_action in keyword_actions {
        let mut message_matches_action = false;

        let triggers = keyword_action
            .triggers
            .as_ref()
            .expect("keyword_action missing triggers");
        if triggers.iter().any(|t| t == "mention") {
            let mentioned_user = keyword_action
                .mentioned_user
                .as_ref()
                .expect("keyword_action missing mentioned_user");
            if incoming_message.mentions_user_id(UserId::new(*mentioned_user)) {
                message_matches_action = true;
            }
        }

        let keywords = keyword_action
            .keywords
            .as_ref()
            .expect("keyword_action missing keywords");
        let regex_keyword_group = keywords.join(r"( |[\?\.',]|$)|(^| )");
        let re =
            Regex::new(&format!("(^| ){regex_keyword_group}( |[\\?\\.',]|$)")).unwrap();
        if re.is_match(&incoming_message.content) {
            message_matches_action = true;
        }

        if message_matches_action {
            let random_action = keyword_action
                .actions
                .as_ref()
                .expect("keyword_action missing actions")
                .choose(&mut rand::thread_rng())
                .expect("keyword_action has empty actions list");
            let action_name = keyword_action
                .name
                .as_deref()
                .expect("keyword_action missing name");

            if let Some(emotes) = random_action.emotes.as_ref() {
                process_emotes_action(ctx, &incoming_message, emotes, action_name).await;
            }
            let mut sending_embed_message = false;
            if let Some(file) = random_action.file.as_ref() {
                sending_embed_message = true;
                process_file_action(
                    ctx,
                    &incoming_message,
                    &random_action.message,
                    file,
                    action_name,
                    file_base_dir,
                )
                .await;
            }
            if let Some(message) = random_action.message.as_ref() {
                if !sending_embed_message {
                    process_message_action(ctx, &incoming_message, message, action_name).await;
                }
            }
            if let Some(message) = random_action.mention.as_ref() {
                process_mention_action(ctx, &incoming_message, message, action_name).await;
            }
        }
    }
}

fn convert_message_list_to_history(
    bot_id: u64,
    message_list: Vec<Message>,
) -> Vec<(String, String, String)> {
    message_list
        .into_iter()
        .filter(|m| !m.content.is_empty())
        .map(|m| {
            (
                m.timestamp.to_rfc3339().unwrap(),
                m.author.name,
                m.content.replace(&format!("<@{}>", bot_id), "ponyboy"),
            )
        })
        .collect()
}

async fn process_emotes_action(
    ctx: &Context,
    incoming_message: &Message,
    emotes: &[String],
    action_name: &str,
) {
    for emote in emotes {
        if let Ok(emote_id) = emote.parse::<u64>() {
            let emoji_id = serenity::all::EmojiId::new(emote_id);
            let emoji = incoming_message
                .guild_id
                .unwrap()
                .emoji(ctx, emoji_id)
                .await
                .unwrap();
            if let Err(why) = incoming_message.react(ctx, emoji).await {
                println!("Error sending message: {why:?}");
            }
        } else if let Err(why) = incoming_message
            .react(ctx, ReactionType::Unicode(emote.clone()))
            .await
        {
            println!("Error sending message: {why:?}");
        }
    }
    println!("{}: emote - {:#?}", action_name, emotes);
}

async fn process_file_action(
    ctx: &Context,
    incoming_message: &Message,
    action_message: &Option<String>,
    file: &str,
    action_name: &str,
    file_base_dir: &str,
) {
    let attachment = match CreateAttachment::path(
        Path::new(file_base_dir).join("file_embeds").join(file),
    )
    .await
    {
        Ok(a) => a,
        Err(why) => {
            println!("Error creating attachment for {}: {why:?}", file);
            return;
        }
    };

    let mut builder = CreateMessage::new();
    if let Some(msg) = action_message {
        builder = builder.content(msg.as_str());
    }

    if let Err(why) = incoming_message
        .channel_id
        .send_files(&ctx.http, [attachment], builder)
        .await
    {
        println!("Error sending message: {why:?}");
    }
    println!(
        "{}: file_embed - {}",
        action_name,
        action_message.as_deref().unwrap_or(file)
    );
}

async fn process_message_action(
    ctx: &Context,
    incoming_message: &Message,
    message: &str,
    action_name: &str,
) {
    if let Err(why) = incoming_message.channel_id.say(&ctx.http, message).await {
        println!("Error sending message: {why:?}");
    }
    println!("{}: message - {}", action_name, message);
}

async fn process_mention_action(
    ctx: &Context,
    incoming_message: &Message,
    message: &str,
    action_name: &str,
) {
    let mentioned_user = incoming_message.author.mention();
    let formatted_message = message.replace("@mention", &format!("{}", mentioned_user));
    if let Err(why) = incoming_message
        .channel_id
        .say(&ctx.http, &formatted_message)
        .await
    {
        println!("Error sending message: {why:?}");
    }
    println!("{}: message - {}", action_name, message);
}
