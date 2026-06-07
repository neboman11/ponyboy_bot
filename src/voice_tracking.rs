use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serenity::model::id::ChannelId;
use serenity::model::voice::VoiceState;
use serenity::prelude::Context;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

pub type ActiveCalls = Arc<Mutex<HashMap<ChannelId, Instant>>>;

pub fn load_tracked_channel_ids() -> Vec<ChannelId> {
    let raw = match std::env::var("TRACKED_VOICE_CHANNEL_IDS") {
        Ok(v) => v,
        Err(_) => {
            println!("voice_tracking: TRACKED_VOICE_CHANNEL_IDS not set, voice tracking disabled");
            return Vec::new();
        }
    };
    raw.split(',')
        .filter_map(|s| s.trim().parse::<u64>().ok().map(ChannelId::new))
        .collect()
}

pub fn load_log_channel_id() -> Option<ChannelId> {
    std::env::var("CALL_LOG_CHANNEL_ID")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(ChannelId::new)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn append_call_record(
    file_base_dir: &str,
    started_at: u64,
    ended_at: u64,
    duration_secs: u64,
    channel_id: ChannelId,
    channel_name: &str,
) {
    use tokio::fs::OpenOptions;
    let path = std::path::Path::new(file_base_dir).join("call_log.csv");
    let needs_header = !path.exists();

    match OpenOptions::new().create(true).append(true).open(&path).await {
        Ok(mut f) => {
            if needs_header {
                let _ = f
                    .write_all(b"started_at,ended_at,duration_seconds,channel_id,channel_name\n")
                    .await;
            }
            let row = format!(
                "{},{},{},{},{}\n",
                started_at, ended_at, duration_secs, channel_id, channel_name
            );
            if let Err(e) = f.write_all(row.as_bytes()).await {
                println!("voice_tracking: failed to write call_log.csv: {e}");
            }
        }
        Err(e) => println!("voice_tracking: failed to open call_log.csv: {e}"),
    }
}

enum CallEvent {
    Started {
        channel_name: String,
        count: usize,
    },
    Ended {
        channel_id: ChannelId,
        channel_name: String,
        started_at: u64,
        duration_secs: u64,
    },
}

pub async fn handle_voice_state_update(
    ctx: &Context,
    new: VoiceState,
    active_calls: &ActiveCalls,
    tracked_channel_ids: &[ChannelId],
    log_channel_id: Option<ChannelId>,
    file_base_dir: &str,
) {
    let guild_id = match new.guild_id {
        Some(id) => id,
        None => return,
    };

    // Phase 1: collect state under locks — no awaits while locks are held.
    // The tokio mutex is awaited first (before the dashmap lock), so yielding
    // during contention cannot deadlock against serenity's cache writes.
    let mut calls = active_calls.lock().await;

    let events: Vec<CallEvent> = {
        let guild = match ctx.cache.guild(guild_id) {
            Some(g) => g,
            None => return,
        };

        let mut evts = Vec::new();

        for &channel_id in tracked_channel_ids {
            let count = guild
                .voice_states
                .values()
                .filter(|vs| vs.channel_id == Some(channel_id))
                .count();

            let channel_name = guild
                .channels
                .get(&channel_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| channel_id.to_string());

            if count >= 2 && !calls.contains_key(&channel_id) {
                calls.insert(channel_id, Instant::now());
                evts.push(CallEvent::Started { channel_name, count });
            } else if count <= 1 {
                if let Some(start) = calls.remove(&channel_id) {
                    let duration_secs = start.elapsed().as_secs();
                    let ended_at = unix_now();
                    let started_at = ended_at.saturating_sub(duration_secs);
                    evts.push(CallEvent::Ended {
                        channel_id,
                        channel_name,
                        started_at,
                        duration_secs,
                    });
                }
            }
        }

        evts
        // guild (dashmap read lock) drops here
    };

    drop(calls); // release mutex before any await

    // Phase 2: async I/O with no locks held.
    for event in events {
        match event {
            CallEvent::Started { channel_name, count } => {
                let msg = format!("Call started in **{}** ({} participants)", channel_name, count);
                println!("voice_tracking: {}", msg);
                if let Some(ch) = log_channel_id {
                    if let Err(e) = ch.say(&ctx.http, &msg).await {
                        println!("voice_tracking: failed to send log message: {e}");
                    }
                }
            }
            CallEvent::Ended {
                channel_id,
                channel_name,
                started_at,
                duration_secs,
            } => {
                let ended_at = unix_now();
                let msg = format!(
                    "Call ended in **{}** (duration: {}s)",
                    channel_name, duration_secs
                );
                println!("voice_tracking: {}", msg);
                if let Some(ch) = log_channel_id {
                    if let Err(e) = ch.say(&ctx.http, &msg).await {
                        println!("voice_tracking: failed to send log message: {e}");
                    }
                }
                append_call_record(
                    file_base_dir,
                    started_at,
                    ended_at,
                    duration_secs,
                    channel_id,
                    &channel_name,
                )
                .await;
            }
        }
    }
}
