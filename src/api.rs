use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use warp::Filter;

#[derive(Deserialize, Debug)]
struct SendDiscordMessageRequest {
    pub(crate) user_id: u64,
    pub(crate) message: String,
}

#[derive(Serialize, Debug)]
struct DiscrodCreateMessageRequest {
    pub(crate) content: String,
}

#[derive(Serialize, Debug)]
struct DiscordCreateDMRequest {
    pub(crate) recipient_id: u64,
}

#[derive(Deserialize, Debug)]
struct SendDiscordMessageResponse {
    pub(crate) id: String,
}

pub(crate) async fn start_api_server(discord_token: String) {
    let rest_route = warp::post()
        .and(warp::path("send_discord_message"))
        .and(warp::body::json())
        .and_then({
            move |body: SendDiscordMessageRequest| {
                // handle the message
                send_discord_message(discord_token.clone(), body)
            }
        });
    warp::serve(rest_route).run(([0, 0, 0, 0], 8081)).await;
}

async fn send_discord_message(
    discord_token: String,
    body: SendDiscordMessageRequest,
) -> Result<impl warp::Reply, Infallible> {
    let channel_id = match create_discord_dm_channel(discord_token.clone(), body.user_id).await {
        Ok(res) => res,
        Err(err) => return Ok(warp::reply::Response::new(format!("{}", err).into())),
    };

    let client = reqwest::Client::new();
    let req = client
        .post(&format!(
            "https://discord.com/api/channels/{}/messages",
            channel_id
        ))
        .json(&DiscrodCreateMessageRequest {
            content: body.message,
        })
        .header("Authorization", format!("Bot {}", discord_token))
        .header("Content-Type", "application/json");

    let res = match req.send().await {
        Ok(res) => res,
        Err(err) => return Ok(warp::reply::Response::new(format!("{}", err).into())),
    };

    if !res.status().is_success() {
        return Ok(warp::reply::Response::new(res.text().await.unwrap().into()));
    }

    Ok(warp::reply::Response::new("Sent discord message".into()))
}

async fn create_discord_dm_channel(discord_token: String, user_id: u64) -> Result<u64, String> {
    let client = reqwest::Client::new();
    let req = client
        .post("https://discord.com/api/users/@me/channels")
        .json(&DiscordCreateDMRequest {
            recipient_id: user_id,
        })
        .header("Authorization", format!("Bot {}", discord_token))
        .header("Content-Type", "application/json");

    let res = match req.send().await {
        Ok(res) => res,
        Err(err) => return Err(format!("{}", err)),
    };

    if !res.status().is_success() {
        return Err(res.text().await.unwrap());
    }

    Ok(res
        .json::<SendDiscordMessageResponse>()
        .await
        .unwrap()
        .id
        .parse()
        .unwrap())
}
