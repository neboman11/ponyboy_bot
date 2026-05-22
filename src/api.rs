use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use warp::{reject::Rejection, Filter};

#[derive(Deserialize, Debug)]
struct SendDiscordMessageRequest {
    pub(crate) user_id: u64,
    pub(crate) message: String,
}

#[derive(Serialize, Debug)]
struct DiscordCreateMessageRequest {
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

#[derive(Serialize, Debug)]
struct ResponseBase {
    pub(crate) message: String,
}
impl warp::reject::Reject for ResponseBase {}

pub(crate) async fn start_api_server(discord_token: String) {
    let rest_route = warp::post()
        .and(warp::path("send_discord_message"))
        .and(warp::body::json())
        .and_then({
            move |body: SendDiscordMessageRequest| send_discord_message(discord_token.clone(), body)
        })
        .recover(report_invalid);
    warp::serve(rest_route).run(([0, 0, 0, 0], 8081)).await;
}

async fn send_discord_message(
    discord_token: String,
    body: SendDiscordMessageRequest,
) -> Result<impl warp::Reply, Rejection> {
    let client = reqwest::Client::new();

    let channel_id = match create_discord_dm_channel(&client, &discord_token, body.user_id).await {
        Ok(res) => res,
        Err(err) => {
            return Err(warp::reject::custom(ResponseBase {
                message: format!("Unable to create DM with user: {}", err),
            }))
        }
    };

    let res = client
        .post(format!(
            "https://discord.com/api/channels/{}/messages",
            channel_id
        ))
        .json(&DiscordCreateMessageRequest {
            content: body.message,
        })
        .header("Authorization", format!("Bot {}", discord_token))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|err| {
            warp::reject::custom(ResponseBase {
                message: format!("Unable to send message to user: {}", err),
            })
        })?;

    if !res.status().is_success() {
        return Err(warp::reject::custom(ResponseBase {
            message: res.text().await.unwrap_or_default(),
        }));
    }

    Ok(warp::reply::Response::new("Sent discord message".into()))
}

async fn create_discord_dm_channel(
    client: &reqwest::Client,
    discord_token: &str,
    user_id: u64,
) -> Result<u64, String> {
    let res = client
        .post("https://discord.com/api/users/@me/channels")
        .json(&DiscordCreateDMRequest {
            recipient_id: user_id,
        })
        .header("Authorization", format!("Bot {}", discord_token))
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(res.text().await.unwrap_or_default());
    }

    res.json::<SendDiscordMessageResponse>()
        .await
        .map_err(|e| e.to_string())?
        .id
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())
}

async fn report_invalid(r: Rejection) -> Result<impl warp::Reply, Infallible> {
    if let Some(e) = r.find::<ResponseBase>() {
        Ok(warp::reply::with_status(
            warp::reply::json(e),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&String::from("Something bad happened")),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
