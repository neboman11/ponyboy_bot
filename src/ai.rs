use std::env;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
struct LocalAICompletionRequest {
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) stop: Vec<String>,
    pub(crate) temperature: f64,
}

#[derive(Deserialize, Debug)]
struct LocalAICompletionChoices {
    pub(crate) text: String,
}

#[derive(Deserialize, Debug)]
struct LocalAICompletionResponse {
    pub(crate) choices: Vec<LocalAICompletionChoices>,
}

pub(crate) async fn generate_ai_bot_response(
    discord_username: String,
    discord_message: String,
    discord_message_history: Vec<(String, String, String)>,
) -> Result<String, String> {
    let mut prompt = "<im_start>system\nYou are ponyboy, a friendly discord chatbot. ponyboy is snarky, edgy, creative, and kind. ponyboy likes being contrarian and picking sides. ponyboy always has lots to say about any topic and loves being creative and wordy with responses. This is a conversation between multiple users and ponyboy.\n\n".to_string();

    prompt += "Message History\n";
    for (_, user, message) in &discord_message_history {
        prompt += format!("{}: {}\n", user, message).as_str();
    }
    prompt += "<|im_end|>\n";

    prompt += "\n<|im_start|>user\n";
    prompt += format!("{}: {}", discord_username, discord_message,).as_str();
    prompt += "<|im_end|>\n";

    prompt += "\n<|im_start|>assistant\nponyboy:";

    let mut stop_words = vec![
        "</s>".to_string(),
        "ponyboy:".to_string(),
        "class-watcher:".to_string(),
        format!("{}:", discord_username),
        "<|im_end|>".to_string(),
    ];
    for (_, user, _) in discord_message_history {
        stop_words.push(format!("{}:", user));
    }

    let completion_url =
        env::var("COMPLETION_URL").expect("Expected completion URL to be set in the environment");
    let completion_model = env::var("COMPLETION_MODEL")
        .expect("Expected completion model to be set in the environment");
    let completion_api_key = env::var("COMPLETION_API_KEY")
        .expect("Expected completion API key to be set in the environment");
    let client = reqwest::Client::new();
    let req = client
        .post(completion_url)
        .json(&LocalAICompletionRequest {
            prompt,
            stop: stop_words,
            temperature: 1.0,
            model: completion_model,
        })
        .header("Authorization", format!("Bearer {}", completion_api_key))
        .header("Content-Type", "application/json");

    let res = match req.send().await {
        Ok(res) => res,
        Err(err) => return Err(format!("{}", err)),
    };

    if !res.status().is_success() {
        return Err(res.text().await.unwrap());
    }

    let response_choices = res
        .json::<LocalAICompletionResponse>()
        .await
        .unwrap()
        .choices;

    Ok(response_choices[0].text.clone())
}
