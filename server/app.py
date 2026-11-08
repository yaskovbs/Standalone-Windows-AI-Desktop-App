"""Collection server for the opt-in learning programme.

Run:  uvicorn app:app --host 127.0.0.1 --port 8787     (put it behind an HTTPS reverse proxy)
Only redacted text pairs rated helpful by the user are accepted. See README.md for the policy.
"""
from __future__ import annotations

import re
import time
from collections import defaultdict, deque

from fastapi import FastAPI, HTTPException, Request
from fastapi.responses import JSONResponse, PlainTextResponse
from pydantic import BaseModel, Field, field_validator

from redact import redact
from store import Store

TRAINING_SYSTEM = (
    "You are a direct, precise assistant. Think step by step inside "
    "<reasoning> tags, then give the final result inside <answer> tags."
)
UUID_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
MAX_BODY = 64 * 1024
MAX_CHARS = 6000
RATE_PER_HOUR = 120

app = FastAPI(title="LocalAutonomousAI contributions", docs_url=None, redoc_url=None)
store = Store()
_hits: dict[str, deque] = defaultdict(deque)


class Msg(BaseModel):
    role: str
    content: str = Field(max_length=MAX_CHARS)


class Item(BaseModel):
    messages: list[Msg] = Field(min_length=3, max_length=3)

    @field_validator("messages")
    @classmethod
    def roles(cls, v: list[Msg]) -> list[Msg]:
        if [m.role for m in v] != ["system", "user", "assistant"]:
            raise ValueError("roles must be system, user, assistant")
        return v


class Payload(BaseModel):
    install_id: str
    app_version: str = Field(max_length=32)
    items: list[Item] = Field(min_length=1, max_length=5)


@app.middleware("http")
async def guard(request: Request, call_next):
    length = int(request.headers.get("content-length") or 0)
    if length > MAX_BODY:
        return JSONResponse({"detail": "payload too large"}, status_code=413)
    ip = request.client.host if request.client else "?"
    q, now = _hits[ip], time.time()
    while q and now - q[0] > 3600:
        q.popleft()
    if len(q) >= RATE_PER_HOUR:
        return JSONResponse({"detail": "rate limit"}, status_code=429)
    q.append(now)
    return await call_next(request)


@app.post("/v1/contribute")
def contribute(p: Payload):
    if not UUID_RE.match(p.install_id):
        raise HTTPException(400, "bad install id")
    stored = 0
    for item in p.items:
        user, assistant = redact(item.messages[1].content.strip()), redact(item.messages[2].content.strip())
        if not user or len(assistant) < 40:
            continue
        stored += store.add(p.install_id, p.app_version, user, assistant)
    return {"stored": stored}


@app.delete("/v1/contributions/{install_id}")
def withdraw(install_id: str):
    if not UUID_RE.match(install_id):
        raise HTTPException(400, "bad install id")
    return {"deleted": store.delete_install(install_id)}


@app.get("/health")
def health():
    return {"ok": True, "contributions": store.count()}


@app.get("/privacy", response_class=PlainTextResponse)
def privacy():
    return (
        "What is collected: the text of a question and the assistant's answer, only for answers the user "
        "marked as helpful, after automatic removal of emails, phone numbers, keys, IP addresses and user "
        "folder names. A random install id (not linked to any account) is stored so a user can delete "
        "their data. Never collected: videos, audio, screenshots, images, settings, tokens. "
        "Use: training future versions of the assistant. Deletion: from the app (Settings -> Withdraw)."
    )
