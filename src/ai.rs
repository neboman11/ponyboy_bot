use std::env;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
struct LlamacppCompletionRequest {
    pub(crate) prompt: String,
    pub(crate) stop: Vec<String>,
    pub(crate) temperature: f64,
}

#[derive(Deserialize, Debug)]
struct LlamacppCompletionResponse {
    pub(crate) content: String,
}

pub(crate) async fn generate_ai_bot_response(
    discord_username: String,
    discord_message: String,
    discord_message_history: Vec<(String, String, String)>,
) -> Result<String, String> {
    let mut prompt = "SYSTEM: You are ponyboy, a friendly discord chatbot. ponyboy is snarky, edgy, creative, and kind. ponyboy likes being contrarian and picking sides. ponyboy always has lots to say about any topic and loves being creative and wordy with responses. This is a conversation between multiple users and ponyboy.\n".to_string();

    for (timestamp, user, message) in &discord_message_history {
        prompt += format!("{} - {}: {}\n", timestamp, user, message).as_str();
    }

    prompt += format!(
        "{}: {}
ponyboy:",
        discord_username, discord_message,
    )
    .as_str();

    let mut stop_words = vec![
        "</s>".to_string(),
        "ponyboy:".to_string(),
        format!("{}:", discord_username),
    ];
    for (_, user, _) in discord_message_history {
        stop_words.push(format!("{}:", user));
    }

    let completion_url =
        env::var("COMPLETION_URL").expect("Expected completion URL to be set in the environment");
    let client = reqwest::Client::new();
    let req = client
        .post(completion_url)
        .json(&LlamacppCompletionRequest {
            prompt: prompt,
            stop: stop_words,
            temperature: 1.0,
        })
        .header("Content-Type", "application/json");

    let res = match req.send().await {
        Ok(res) => res,
        Err(err) => return Err(format!("{}", err)),
    };

    if !res.status().is_success() {
        return Err(res.text().await.unwrap());
    }

    Ok(res
        .json::<LlamacppCompletionResponse>()
        .await
        .unwrap()
        .content)
}
