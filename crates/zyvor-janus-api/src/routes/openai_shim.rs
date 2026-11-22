// Copyright 2026 ZyvorAI Labs Private Limited
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-2.0

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use async_stream::stream;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
pub struct ChatCompletionRequest {
    model: String,
    #[serde(default)]
    messages: Vec<ChatMessage>,
    #[serde(default)]
    stream: bool,
    #[serde(default = "default_max_tokens")]
    max_tokens: i64,
}

fn default_max_tokens() -> i64 {
    128
}

/// Port of Python's `_require_api_key` -- deliberately its own check, not
/// the blanket `/api/*` bearer middleware (auth.rs), so this route's error
/// text and credential handling stay independent even though today both
/// happen to check the same `ZYVOR_JANUS_API_KEY`.
///
/// A middleware rather than an in-handler check: FastAPI resolves
/// `Depends(_require_api_key)` ahead of Pydantic body validation, so an
/// unauthenticated request with a malformed body still gets 401, not a
/// validation error. Axum extractors (including `Json<T>`) run in the
/// handler's declared order and short-circuit on the first failure, so an
/// in-handler check after a `Json<T>` param would see 422 win that race
/// instead -- a middleware runs before any of the handler's extractors.
pub async fn require_shim_api_key(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let Some(auth_header) = auth_header else {
        return Err(ApiError::unauthorized("missing bearer token"));
    };
    let Some(token) = auth_header.strip_prefix("Bearer ") else {
        return Err(ApiError::unauthorized("missing bearer token"));
    };
    if !constant_time_eq(token.trim().as_bytes(), state.inner.api_key.as_bytes()) {
        return Err(ApiError::unauthorized("invalid api key"));
    }
    Ok(next.run(request).await)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Port of Python's `_check_rate_limit`: a 60-second sliding window per
/// client key, capped at `ZYVOR_JANUS_SHIM_RATE_LIMIT` requests/min.
fn check_rate_limit(state: &AppState, client_key: &str) -> Result<(), ApiError> {
    let now = Instant::now();
    let mut buckets = state.inner.rate_buckets.lock().expect("rate bucket lock");
    let window = buckets.entry(client_key.to_string()).or_default();
    window.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
    if window.len() >= state.inner.shim_rate_limit_per_min as usize {
        return Err(ApiError::too_many_requests("rate limit exceeded"));
    }
    window.push(now);
    Ok(())
}

/// Port of Python's `_estimate_tokens`: input = sum of whitespace-split word
/// counts (min 1 per message) across every message regardless of role;
/// default output = clamp(input / 2, 16, 512).
fn estimate_tokens(messages: &[ChatMessage]) -> (i64, i64) {
    let input_tokens: i64 = messages
        .iter()
        .map(|m| (m.content.split_whitespace().count() as i64).max(1))
        .sum();
    let default_output = (input_tokens / 2).clamp(16, 512);
    (input_tokens, default_output)
}

fn estimate_ttft_ms(state: &AppState, model: &str, input_tokens: i64, output_tokens: i64) -> f64 {
    let (prefill_ms, decode_tps) = state.inner.model_profiles.lookup_v2(model, "H100");
    let ttft = input_tokens as f64 * prefill_ms;
    let decode_secs = output_tokens as f64 / decode_tps.max(1.0);
    ttft + decode_secs * 1000.0 * 0.05
}

fn chat_id() -> String {
    format!("chatcmpl-{}", &Uuid::new_v4().simple().to_string()[..8])
}

#[derive(Serialize)]
struct ChatChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
}

#[derive(Serialize)]
struct ChatChunkChoice {
    index: u32,
    delta: ChatChunkDelta,
    finish_reason: Option<&'static str>,
}

#[derive(Serialize)]
struct ChatChunk {
    id: String,
    object: &'static str,
    choices: Vec<ChatChunkChoice>,
}

/// Port of Python's `_stream_tokens`: sleeps `ttft_ms`, then emits one SSE
/// chunk per whitespace-split word (20ms apart), a final `finish_reason:
/// "stop"` chunk, then the `[DONE]` sentinel.
fn stream_tokens(
    content: String,
    ttft_ms: f64,
) -> impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>> {
    stream! {
        tokio::time::sleep(Duration::from_secs_f64((ttft_ms / 1000.0).max(0.0))).await;
        let mut words: Vec<&str> = content.split_whitespace().collect();
        if words.is_empty() {
            words.push("OK");
        }
        for word in words {
            let chunk = ChatChunk {
                id: chat_id(),
                object: "chat.completion.chunk",
                choices: vec![ChatChunkChoice {
                    index: 0,
                    delta: ChatChunkDelta { content: Some(format!("{word} ")) },
                    finish_reason: None,
                }],
            };
            yield Ok(Event::default().data(serde_json::to_string(&chunk).unwrap_or_default()));
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let done_chunk = ChatChunk {
            id: chat_id(),
            object: "chat.completion.chunk",
            choices: vec![ChatChunkChoice {
                index: 0,
                delta: ChatChunkDelta { content: None },
                finish_reason: Some("stop"),
            }],
        };
        yield Ok(Event::default().data(serde_json::to_string(&done_chunk).unwrap_or_default()));
        yield Ok(Event::default().data("[DONE]"));
    }
}

#[derive(Serialize)]
struct ChatMessageOut {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatChoice {
    index: u32,
    message: ChatMessageOut,
    finish_reason: &'static str,
}

#[derive(Serialize)]
struct Usage {
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

#[derive(Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    model: String,
    choices: Vec<ChatChoice>,
    usage: Usage,
}

/// `POST /v1/chat/completions` -- OpenAI-compatible virtual endpoint backed
/// by no real model; timing is simulated via `estimate_ttft_ms`, the "reply"
/// echoes any `assistant`-role messages in the request (or a fixed fallback
/// string), matching Python's `openai_shim.py` exactly, hallucination and
/// all -- this endpoint never calls a real LLM in either implementation.
pub async fn chat_completions(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<ChatCompletionRequest>,
) -> Result<Response, ApiError> {
    check_rate_limit(&state, &addr.ip().to_string())?;

    let (input_tokens, default_output) = estimate_tokens(&body.messages);
    let output_tokens = if body.max_tokens != 0 {
        body.max_tokens
    } else {
        default_output
    };
    let ttft_ms = estimate_ttft_ms(&state, &body.model, input_tokens, output_tokens);
    let reply = {
        let joined = body
            .messages
            .iter()
            .filter(|m| m.role == "assistant")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if joined.is_empty() {
            "Zyvor Janus virtual completion.".to_string()
        } else {
            joined
        }
    };

    if body.stream {
        return Ok(Sse::new(stream_tokens(reply, ttft_ms)).into_response());
    }

    tokio::time::sleep(Duration::from_secs_f64((ttft_ms / 1000.0).max(0.0))).await;
    Ok(Json(ChatCompletionResponse {
        id: chat_id(),
        object: "chat.completion",
        model: body.model,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessageOut {
                role: "assistant",
                content: reply,
            },
            finish_reason: "stop",
        }],
        usage: Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
        },
    })
    .into_response())
}
