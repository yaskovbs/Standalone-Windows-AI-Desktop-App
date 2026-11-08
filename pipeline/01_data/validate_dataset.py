"""Phase 1 - Validation: filter malformed, oversized, duplicate or template-broken rows."""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import config  # noqa: E402

REASONING_RE = re.compile(r"<reasoning>.+?</reasoning>\s*<answer>.+?</answer>\s*$", re.S)
ROLES = ["system", "user", "assistant"]
SUSPICIOUS = ("as an ai language model", "i cannot browse", "lorem ipsum", "[insert")


FENCE_RE = re.compile(r"```([A-Za-z0-9_+-]*)\n(.*?)```", re.S)


def code_blocks(text: str) -> list[tuple[str, str]]:
    return [(lang.lower(), body) for lang, body in FENCE_RE.findall(text)]


def check_code(assistant: str) -> str | None:
    """Syntax-check Python and JSON blocks; code that does not even parse must not be learned."""
    for lang, body in code_blocks(assistant):
        if lang in ("python", "py"):
            try:
                ast.parse(body)
            except SyntaxError:
                return "bad_python_syntax"
        elif lang == "json":
            try:
                json.loads(body)
            except json.JSONDecodeError:
                return "bad_json_block"
    if assistant.count("```") % 2:
        return "unclosed_code_fence"
    return None


def run_python_tests(assistant: str, timeout: int = 10) -> str | None:
    """Execute Python blocks that contain assert-based tests. WARNING: runs model-written code."""
    for lang, body in code_blocks(assistant):
        if lang not in ("python", "py") or "assert" not in body:
            continue
        with tempfile.TemporaryDirectory() as tmp:
            try:
                proc = subprocess.run(
                    [sys.executable, "-I", "-c", body], cwd=tmp, capture_output=True, timeout=timeout, text=True
                )
            except subprocess.TimeoutExpired:
                return "execution_timeout"
        if proc.returncode != 0:
            return "failed_execution"
    return None


def check_row(row: dict) -> str | None:
    """Return a rejection reason, or None when the row is valid."""
    msgs = row.get("messages")
    if not isinstance(msgs, list) or [m.get("role") for m in msgs] != ROLES:
        return "bad_roles"
    if any(not isinstance(m.get("content"), str) or not m["content"].strip() for m in msgs):
        return "empty_content"
    assistant = msgs[2]["content"]
    if len(assistant) < config.MIN_CHARS_ASSISTANT:
        return "too_short"
    if sum(len(m["content"]) for m in msgs) > config.MAX_CHARS_PER_ROW:
        return "too_long"
    if not REASONING_RE.search(assistant):
        return "template_mismatch"
    lowered = assistant.lower()
    if any(s in lowered for s in SUSPICIOUS):
        return "suspicious_text"
    if assistant.count("<reasoning>") != 1 or assistant.count("<answer>") != 1:
        return "tag_count"
    return check_code(assistant)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--execute",
        action="store_true",
        help="run assert-based Python tests from the samples (executes untrusted code: use a VM/container)",
    )
    args = parser.parse_args()
    if not config.RAW_DATASET.exists():
        sys.exit(f"Missing {config.RAW_DATASET}; run generate_dataset.py first.")

    seen: set[str] = set()
    rejected: Counter[str] = Counter()
    kept = 0

    with open(config.RAW_DATASET, encoding="utf-8") as src, open(
        config.CLEAN_DATASET, "w", encoding="utf-8"
    ) as dst:
        for line in src:
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                rejected["bad_json"] += 1
                continue
            reason = check_row(row)
            if reason is None and args.execute:
                reason = run_python_tests(row["messages"][2]["content"])
            if reason:
                rejected[reason] += 1
                continue
            key = hashlib.sha1(row["messages"][1]["content"].lower().encode()).hexdigest()
            if key in seen:
                rejected["duplicate"] += 1
                continue
            seen.add(key)
            dst.write(json.dumps({"messages": row["messages"]}, ensure_ascii=False) + "\n")
            kept += 1

    print(f"Kept {kept} rows -> {config.CLEAN_DATASET}")
    for reason, count in rejected.most_common():
        print(f"  rejected {count:>5}  {reason}")


if __name__ == "__main__":
    main()
