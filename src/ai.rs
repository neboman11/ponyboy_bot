use std::env;

use reqwest::Client;
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

#[derive(Deserialize, Debug)]
struct ConfigurationSetting {
    // pub(crate) section: String,
    // pub(crate) name: String,
    pub(crate) value: String,
}

pub(crate) async fn generate_ai_bot_response(
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

    let mut prompt = format!("<im_start>system\n{}\n\n", base_prompt);

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
    let completion_model = match fetch_config_setting(
        &client,
        format!("ponyboy"),
        format!("completion_model"),
    )
    .await
    {
        Ok(base_prompt) => base_prompt,
        Err(err) => return Err(err),
    };
    let completion_api_key = env::var("COMPLETION_API_KEY")
        .expect("Expected completion API key to be set in the environment");
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
