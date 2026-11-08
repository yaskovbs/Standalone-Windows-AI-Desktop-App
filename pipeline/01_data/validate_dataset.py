"""Phase 1 - Validation: filter malformed, oversized, duplicate or template-broken rows."""
from __future__ import annotations

import hashlib
import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import config  # noqa: E402

REASONING_RE = re.compile(r"<reasoning>.+?</reasoning>\s*<answer>.+?</answer>\s*$", re.S)
ROLES = ["system", "user", "assistant"]
SUSPICIOUS = ("as an ai language model", "i cannot browse", "lorem ipsum", "[insert")


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
    return None


def main() -> None:
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
