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
    discord_message_history: String,
) -> Result<String, String> {
    let prompt = format!("This is a conversation between {user} and ponyboy, a friendly discord chatbot. ponyboy is snarky, edgy, aggressive, and kind. ponyboy likes being contrarian and picking sides. ponyboy always has lots to say about any topic and loves being creative and wordy with responses.

{history}
{user}: {message}
ponyboy:", user = discord_username, message = discord_message, history = discord_message_history);

    let completion_url =
        env::var("COMPLETION_URL").expect("Expected completion URL to be set in the environment");
    let client = reqwest::Client::new();
    let req = client
        .post(completion_url)
        .json(&LlamacppCompletionRequest {
            prompt: prompt,
            stop: vec![
                "</s>".to_string(),
                "ponyboy:".to_string(),
                format!("{}:", discord_username),
            ],
            temperature: 1.1,
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
