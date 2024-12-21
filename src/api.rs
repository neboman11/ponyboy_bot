use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use warp::{reject::Rejection, Filter};

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
            move |body: SendDiscordMessageRequest| {
                // handle the message
                send_discord_message(discord_token.clone(), body)
            }
        })
        .recover(report_invalid);
    warp::serve(rest_route).run(([0, 0, 0, 0], 8081)).await;
}

async fn send_discord_message(
    discord_token: String,
    body: SendDiscordMessageRequest,
) -> Result<impl warp::Reply, Rejection> {
    let channel_id = match create_discord_dm_channel(discord_token.clone(), body.user_id).await {
        Ok(res) => res,
        Err(err) => {
            return Err(warp::reject::custom(ResponseBase {
                message: format!("Unable to create DM with user: {}", err).into(),
            }))
        }
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
        Err(err) => {
            return Err(warp::reject::custom(ResponseBase {
                message: format!("Unable to send message to user: {}", err).into(),
            }))
        }
    };

    if !res.status().is_success() {
        return Err(warp::reject::custom(ResponseBase {
            message: res.text().await.unwrap().into(),
        }));
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

async fn report_invalid(r: Rejection) -> Result<impl warp::Reply, Infallible> {
    if let Some(e) = r.find::<ResponseBase>() {
        // It was our specific error type, do whatever we want. We
        // will just print out the error text.
        Ok(warp::reply::with_status(
            warp::reply::json(e),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else {
        // Do prettier error reporting for the default error here.
        Ok(warp::reply::with_status(
            warp::reply::json(&String::from("Something bad happened")),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
