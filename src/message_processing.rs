use serenity::all::{Context, GetMessages, Message};

use crate::ai;

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
