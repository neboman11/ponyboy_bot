use std::collections::HashMap;
use std::sync::Arc;

use serenity::http::Http;
use serenity::model::id::ChannelId;
use serenity::model::voice::VoiceState;
use serenity::prelude::Context;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Maps channel ID to Unix timestamp of call start.
pub type ActiveCalls = Arc<Mutex<HashMap<ChannelId, u64>>>;
// Maps channel ID to the grace-period task waiting to officially end the call.
pub type PendingEnds = Arc<Mutex<HashMap<ChannelId, JoinHandle<()>>>>;

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

pub fn load_grace_period_secs() -> u64 {
    std::env::var("CALL_GRACE_PERIOD_SECS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    match (h, m) {
        (0, 0) => format!("{}s", s),
        (0, _) => format!("{}m {}s", m, s),
        _ => format!("{}h {}m {}s", h, m, s),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn restore_active_calls(file_base_dir: &str) -> HashMap<ChannelId, u64> {
    let path = std::path::Path::new(file_base_dir).join("active_calls.json");
    let json = match tokio::fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    match serde_json::from_str::<HashMap<String, u64>>(&json) {
        Ok(map) => {
            let calls: HashMap<ChannelId, u64> = map
                .into_iter()
                .filter_map(|(k, v)| k.parse::<u64>().ok().map(|id| (ChannelId::new(id), v)))
                .collect();
            println!("voice_tracking: restored {} active call(s) from disk", calls.len());
            calls
        }
        Err(e) => {
            println!("voice_tracking: failed to parse active_calls.json: {e}");
            HashMap::new()
        }
    }
}

async fn persist_active_calls(file_base_dir: &str, calls: &HashMap<ChannelId, u64>) {
    let path = std::path::Path::new(file_base_dir).join("active_calls.json");
    let map: HashMap<String, u64> = calls.iter().map(|(k, v)| (k.get().to_string(), *v)).collect();
    match serde_json::to_string(&map) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&path, json).await {
                println!("voice_tracking: failed to save active_calls.json: {e}");
            }
        }
        Err(e) => println!("voice_tracking: failed to serialize active_calls: {e}"),
    }
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

// Spawned as a task when a call drops to ≤1 participant. Sleeps for the grace
// period, then officially ends the call. Aborted if someone rejoins in time.
async fn end_call_task(
    active_calls: ActiveCalls,
    pending_ends: PendingEnds,
    channel_id: ChannelId,
    channel_name: String,
    log_channel_id: Option<ChannelId>,
    http: Arc<Http>,
    file_base_dir: String,
    grace_secs: u64,
) {
    tokio::time::sleep(std::time::Duration::from_secs(grace_secs)).await;

    // Lock ordering: active_calls → pending_ends (matches event handler).
    let started_at = {
        let mut calls = active_calls.lock().await;
        calls.remove(&channel_id)
    };
    {
        let mut pending = pending_ends.lock().await;
        pending.remove(&channel_id);
    }

    let Some(started_at) = started_at else { return };

    let ended_at = unix_now();
    let duration_secs = ended_at.saturating_sub(started_at);

    {
        let calls = active_calls.lock().await;
        persist_active_calls(&file_base_dir, &calls).await;
    }

    let msg = format!(
        "Call ended in **{}** (duration: {})",
        channel_name,
        format_duration(duration_secs)
    );
    println!("voice_tracking: {}", msg);
    if let Some(ch) = log_channel_id {
        if let Err(e) = ch.say(&http, &msg).await {
            println!("voice_tracking: failed to send log message: {e}");
        }
    }
    append_call_record(&file_base_dir, started_at, ended_at, duration_secs, channel_id, &channel_name).await;
}

enum CallEvent {
    Started { channel_name: String, count: usize },
    Resumed { channel_name: String },
}

pub async fn handle_voice_state_update(
    ctx: &Context,
    new: VoiceState,
    active_calls: &ActiveCalls,
    pending_ends: &PendingEnds,
    tracked_channel_ids: &[ChannelId],
    log_channel_id: Option<ChannelId>,
    file_base_dir: &str,
    grace_period_secs: u64,
) {
    let guild_id = match new.guild_id {
        Some(id) => id,
        None => return,
    };

    // Phase 1: collect state under locks — no awaits while locks are held.
    // Lock ordering: active_calls → pending_ends → dashmap guild read lock.
    let mut calls = active_calls.lock().await;
    let mut pending = pending_ends.lock().await;

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

            if count >= 2 {
                if calls.contains_key(&channel_id) {
                    // Ongoing or in grace period — cancel any pending end.
                    if let Some(handle) = pending.remove(&channel_id) {
                        handle.abort();
                        evts.push(CallEvent::Resumed { channel_name });
                    }
                } else {
                    // New call.
                    calls.insert(channel_id, unix_now());
                    evts.push(CallEvent::Started { channel_name, count });
                }
            } else if count <= 1 && calls.contains_key(&channel_id) && !pending.contains_key(&channel_id) {
                // Start grace period — channel stays in active_calls until task fires.
                let handle = tokio::spawn(end_call_task(
                    active_calls.clone(),
                    pending_ends.clone(),
                    channel_id,
                    channel_name,
                    log_channel_id,
                    ctx.http.clone(),
                    file_base_dir.to_string(),
                    grace_period_secs,
                ));
                pending.insert(channel_id, handle);
            }
        }

        evts
        // guild (dashmap read lock) drops here
    };

    let calls_snapshot = calls.clone();
    drop(pending);
    drop(calls);

    // Phase 2: async I/O with no locks held.
    // End-of-call work is handled by end_call_task; only persist start/resume here.
    persist_active_calls(file_base_dir, &calls_snapshot).await;

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
            CallEvent::Resumed { channel_name } => {
                println!("voice_tracking: call in {} resumed within grace period", channel_name);
            }
        }
    }
}
