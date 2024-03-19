use std::env;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct LocalAICompletionMessage {
    pub(crate) content: String,
    pub(crate) name: String,
    pub(crate) role: String,
}

#[derive(Serialize, Debug)]
struct LocalAICompletionRequest {
    pub(crate) messages: Vec<LocalAICompletionMessage>,
    pub(crate) model: String,
    pub(crate) stop: Vec<String>,
    pub(crate) temperature: f64,
}

#[derive(Deserialize, Debug)]
struct LocalAICompletionChoices {
    pub(crate) message: LocalAICompletionMessage,
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
    let mut messages = Vec::new();
    messages.push(LocalAICompletionMessage{
        content: "You are ponyboy, a friendly discord chatbot. ponyboy is snarky, edgy, creative, and kind. ponyboy likes being contrarian and picking sides. ponyboy always has lots to say about any topic and loves being creative and wordy with responses. This is a conversation between multiple users and ponyboy.\n".to_string(),
        name: "SYSTEM".to_string(),
        role: "SYSTEM".to_string(),
    });

    for (timestamp, user, message) in &discord_message_history {
        if user == "ponyboy" {
            messages.push(LocalAICompletionMessage {
                content: format!("{} - {}: {}\n", timestamp, user, message),
                name: user.clone(),
                role: "ASSISTANT".to_string(),
            });
        }
        messages.push(LocalAICompletionMessage {
            content: format!("{} - {}: {}\n", timestamp, user, message),
            name: user.clone(),
            role: "USER".to_string(),
        });
    }

    messages.push(LocalAICompletionMessage {
        content: discord_message,
        name: discord_username.clone(),
        role: "USER".to_string(),
    });

    //     messages += format!(
    //         "{}: {}
    // ponyboy:",
    //         discord_username, discord_message,
    //     )
    //     .as_str();

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
    let completion_model = env::var("COMPLETION_MODEL")
        .expect("Expected completion model to be set in the environment");
    let client = reqwest::Client::new();
    let req = client
        .post(completion_url)
        .json(&LocalAICompletionRequest {
            messages,
            stop: stop_words,
            temperature: 1.0,
            model: completion_model,
        })
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

    Ok(response_choices[0].message.content.clone())
}
