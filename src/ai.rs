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
    let base_prompt =
        match fetch_config_setting(&client, format!("ponyboy"), format!("base_prompt")).await {
            Ok(base_prompt) => base_prompt,
            Err(err) => return Err(err),
        };

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

    let group_members = unique_users.join(", ");
    messages.push(OpenAIMessage {
        role: "system".to_string(),
        content: format!("[Start a new group chat. Group members: {}]", group_members),
    });
    for (_, user, message) in &discord_message_history {
        if user.eq(&bot_username) {
            messages.push(OpenAIMessage {
                role: "assistant".to_string(),
                content: format!("{}", message),
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

    let completion_base_url =
        match fetch_config_setting(&client, format!("ponyboy"), format!("openai_base_url")).await {
            Ok(completion_base_url) => completion_base_url,
            Err(err) => return Err(err),
        };

    let completion_model = match fetch_config_setting(
        &client,
        format!("ponyboy"),
        format!("completion_model"),
    )
    .await
    {
        Ok(completion_model) => completion_model,
        Err(err) => return Err(err),
    };
    let completion_api_key = env::var("COMPLETION_API_KEY")
        .expect("Expected completion API key to be set in the environment");

    let req = client
        .post(completion_base_url + "/v1/chat/completions")
        .json(&OpenAIChatRequest {
            model: completion_model,
            messages,
            temperature: 1.0,
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

    let response_choices = res.json::<OpenAIChatResponse>().await.unwrap().choices;

    Ok(response_choices[0].message.content.clone())
}

async fn fetch_config_setting(
    client: &Client,
    section: String,
    name: String,
) -> Result<String, String> {
    let config_settings_url = match env::var("CONFIG_SETTINGS_URL") {
        Ok(config_setting_url) => config_setting_url,
        Err(_) => {
            return Err(
                "Expected configuration settings service URL to be set in the environment"
                    .to_string(),
            )
        }
    };
    let config_settings_path = format!("configuration_setting/{}/{}", section, name);

    let req = client.get(format!("{}{}", config_settings_url, config_settings_path));

    let res = match req.send().await {
        Ok(res) => res,
        Err(err) => return Err(format!("{}", err)),
    };

    if !res.status().is_success() {
        return Err(res.text().await.unwrap());
    }

    let value = match res.json::<ConfigurationSetting>().await {
        Ok(config_setting) => config_setting.value,
        Err(err) => return Err(format!("{}", err)),
    };

    Ok(value)
}
