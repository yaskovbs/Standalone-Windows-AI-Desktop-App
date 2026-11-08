"""Server-side redaction: a second pass, because the client can never be fully trusted."""
from __future__ import annotations

import re

SECRETS = re.compile(
    r"\b(?:sk-[A-Za-z0-9_\-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}"
    r"|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9\-]{10,}|AIza[0-9A-Za-z_\-]{30,})"
)
BEARER = re.compile(r"bearer\s+[A-Za-z0-9._\-]{20,}", re.I)
KEY_VALUE = re.compile(r"\b(password|passwd|pwd|secret|token|api[_-]?key)(\s*[:=]\s*)[^\s\"',;]+", re.I)
EMAIL = re.compile(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}")
LONG_TOKEN = re.compile(r"\b[A-Za-z0-9+/=_\-]{40,}\b")
WIN_USER = re.compile(r"([A-Za-z]:\\Users\\)[^\\\s\"'<>|]+", re.I)
UNIX_HOME = re.compile(r"(/home/|/Users/)[^/\s\"']+")
IPV4 = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
NUMBERS = re.compile(r"\+?\d[\d\-\s().]{7,}\d")


def _numbers(m: re.Match) -> str:
    return "[NUMBER]" if sum(c.isdigit() for c in m.group(0)) >= 9 else m.group(0)


def redact(text: str) -> str:
    text = SECRETS.sub("[SECRET]", text)
    text = BEARER.sub("Bearer [SECRET]", text)
    text = KEY_VALUE.sub(r"\1\2[SECRET]", text)
    text = EMAIL.sub("[EMAIL]", text)
    text = LONG_TOKEN.sub("[SECRET]", text)
    text = WIN_USER.sub(r"\1<user>", text)
    text = UNIX_HOME.sub(r"\1<user>", text)
    text = IPV4.sub("[IP]", text)
    return NUMBERS.sub(_numbers, text)
