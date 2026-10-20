# Copyright 2026 ZyvorAI Labs Private Limited
# SPDX-License-Identifier: Apache-2.0

"""OpenAI-compatible virtual endpoint backed by Zyvor Janus inference model.

Superseded by crates/zyvor-janus-api/src/routes/openai_shim.rs -- see the
note at the top of server/app.py.
"""

from __future__ import annotations

import asyncio
import hmac
import json
import os
import time
import uuid
from collections import defaultdict
from typing import Any, AsyncIterator

from fastapi import APIRouter, Depends, Header, HTTPException, Request
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field

from zyvor_janus.adapters.profiles import ProfileRegistry

router = APIRouter(prefix="/v1", tags=["openai-shim"])

DEFAULT_API_KEY = os.environ.get("ZYVOR_JANUS_API_KEY", "dev-zyvor-janus-key")
RATE_LIMIT_PER_MIN = int(os.environ.get("ZYVOR_JANUS_SHIM_RATE_LIMIT", "120"))
PROFILES_DIR = os.environ.get("ZYVOR_JANUS_PROFILES_DIR", "configs/profiles")

_rate_buckets: dict[str, list[float]] = defaultdict(list)
_profile_registry = ProfileRegistry(__import__("pathlib").Path(PROFILES_DIR))


class ChatMessage(BaseModel):
    role: str
    content: str


class ChatCompletionRequest(BaseModel):
    model: str
    messages: list[ChatMessage] = Field(default_factory=list)
    stream: bool = False
    max_tokens: int = 128


def _estimate_tokens(messages: list[ChatMessage]) -> tuple[int, int]:
    input_tokens = sum(max(1, len(m.content.split())) for m in messages)
    return input_tokens, max(16, min(512, input_tokens // 2))


def _require_api_key(authorization: str | None = Header(default=None)) -> str:
    if not authorization or not authorization.startswith("Bearer "):
        raise HTTPException(status_code=401, detail="missing bearer token")
    token = authorization.removeprefix("Bearer ").strip()
    if not hmac.compare_digest(token.encode(), DEFAULT_API_KEY.encode()):
        raise HTTPException(status_code=401, detail="invalid api key")
    return token


def _check_rate_limit(client_key: str) -> None:
    now = time.time()
    window = _rate_buckets[client_key]
    _rate_buckets[client_key] = [t for t in window if now - t < 60.0]
    if len(_rate_buckets[client_key]) >= RATE_LIMIT_PER_MIN:
        raise HTTPException(status_code=429, detail="rate limit exceeded")
    _rate_buckets[client_key].append(now)


def _estimate_ttft_ms(model: str, input_tokens: int, output_tokens: int) -> float:
    try:
        prefill_ms, decode_tps = _profile_registry.lookup_v2(model, "H100")
    except Exception:
        prefill_ms, decode_tps = 0.08, 120.0
    ttft = input_tokens * prefill_ms
    decode_secs = output_tokens / max(decode_tps, 1.0)
    return ttft + decode_secs * 1000.0 * 0.05


async def _stream_tokens(content: str, ttft_ms: float) -> AsyncIterator[str]:
    await asyncio.sleep(ttft_ms / 1000.0)
    words = content.split()
    if not words:
        words = ["OK"]
    for word in words:
        chunk = {
            "id": f"chatcmpl-{uuid.uuid4().hex[:8]}",
            "object": "chat.completion.chunk",
            "choices": [{"index": 0, "delta": {"content": word + " "}, "finish_reason": None}],
        }
        yield f"data: {json.dumps(chunk)}\n\n"
        await asyncio.sleep(0.02)
    done = {
        "id": f"chatcmpl-{uuid.uuid4().hex[:8]}",
        "object": "chat.completion.chunk",
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
    }
    yield f"data: {json.dumps(done)}\n\n"
    yield "data: [DONE]\n\n"


@router.post("/chat/completions")
async def chat_completions(
    body: ChatCompletionRequest,
    request: Request,
    _token: str = Depends(_require_api_key),
) -> Any:
    client_key = request.client.host if request.client else "local"
    _check_rate_limit(client_key)
    input_tokens, default_output = _estimate_tokens(body.messages)
    output_tokens = body.max_tokens or default_output
    ttft_ms = _estimate_ttft_ms(body.model, input_tokens, output_tokens)
    reply = " ".join(m.content for m in body.messages if m.role == "assistant") or "Zyvor Janus virtual completion."
    if body.stream:
        return StreamingResponse(_stream_tokens(reply, ttft_ms), media_type="text/event-stream")
    await asyncio.sleep(ttft_ms / 1000.0)
    return {
        "id": f"chatcmpl-{uuid.uuid4().hex[:8]}",
        "object": "chat.completion",
        "model": body.model,
        "choices": [
            {
                "index": 0,
                "message": {"role": "assistant", "content": reply},
                "finish_reason": "stop",
            }
        ],
        "usage": {
            "prompt_tokens": input_tokens,
            "completion_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens,
        },
    }
