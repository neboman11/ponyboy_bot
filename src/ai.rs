use std::env;

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    temperature: f64,
}

#[derive(Deserialize, Serialize, Debug)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChatChoice>,
}

#[derive(Deserialize, Debug)]
struct OpenAIChatChoice {
    message: OpenAIMessage,
}

#[derive(Deserialize, Debug)]
struct ConfigurationSetting {
    value: String,
}

pub(crate) async fn generate_ai_bot_response(
    bot_username: String,
    discord_username: String,
    discord_message: String,
    discord_message_history: Vec<(String, String, String)>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let base_prompt = fetch_config_setting(&client, "ponyboy", "base_prompt").await?;

    let mut messages = vec![OpenAIMessage {
        role: "system".to_string(),
        content: base_prompt,
    }];

    let mut unique_users: Vec<String> = discord_message_history
        .iter()
        .map(|(_, user, _)| user.clone())
        .collect();
    unique_users.sort();
    unique_users.dedup();

    messages.push(OpenAIMessage {
        role: "system".to_string(),
        content: format!("[Start a new group chat. Group members: {}]", unique_users.join(", ")),
    });

    for (_, user, message) in &discord_message_history {
        if user == &bot_username {
            messages.push(OpenAIMessage {
                role: "assistant".to_string(),
                content: message.clone(),
            });
        } else {
            messages.push(OpenAIMessage {
                role: "user".to_string(),
                content: format!("{}: {}", user, message),
            });
        }
    }

    messages.push(OpenAIMessage {
        role: "user".to_string(),
        content: format!("{}: {}", discord_username, discord_message),
    });

    messages.push(OpenAIMessage {
        role: "system".to_string(),
        content: format!("[Write the next reply only as {}.]", bot_username),
    });

    let completion_base_url = fetch_config_setting(&client, "ponyboy", "openai_base_url").await?;
    let completion_model = fetch_config_setting(&client, "ponyboy", "completion_model").await?;
    let completion_api_key = env::var("COMPLETION_API_KEY")
        .map_err(|_| "Expected completion API key to be set in the environment".to_string())?;

    let res = client
        .post(format!("{}/v1/chat/completions", completion_base_url))
        .json(&OpenAIChatRequest {
            model: completion_model,
            messages,
            temperature: 1.0,
        })
        .header("Authorization", format!("Bearer {}", completion_api_key))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(res.text().await.unwrap_or_default());
    }

    let response = res
        .json::<OpenAIChatResponse>()
        .await
        .map_err(|e| e.to_string())?;

    response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "No choices returned in response".to_string())
}

async fn fetch_config_setting(client: &Client, section: &str, name: &str) -> Result<String, String> {
    let config_settings_url = env::var("CONFIG_SETTINGS_URL").map_err(|_| {
        "Expected configuration settings service URL to be set in the environment".to_string()
    })?;

    let res = client
        .get(format!(
            "{}configuration_setting/{}/{}",
            config_settings_url, section, name
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(res.text().await.unwrap_or_default());
    }

    res.json::<ConfigurationSetting>()
        .await
        .map(|s| s.value)
        .map_err(|e| e.to_string())
}
