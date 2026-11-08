"""Phase 1 - Teacher stage: generate synthetic CoT instruction data with Claude.

Usage:
    export ANTHROPIC_API_KEY=...
    python 01_data/generate_dataset.py --calls 20
"""
from __future__ import annotations

import argparse
import json
import random
import sys
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import anthropic
from tqdm import tqdm

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import config  # noqa: E402

GENERATOR_PROMPT = """You are building a fine-tuning dataset for a small assistant.
Create {n} DIVERSE, self-contained training examples in the category:
"{category}" -- {description}.

Rules:
- Vary difficulty, domain, phrasing and length. Seed for variety: {seed}.
- Each assistant reply MUST be formatted exactly as:
  <reasoning>explicit step-by-step reasoning</reasoning>
  <answer>the final, concise answer</answer>
- Be factually correct. Never invent APIs, quotes or statistics.
- For web_grounded examples, put numbered snippets in the user message
  and cite only facts present in them.
- Return ONLY a JSON array. Each item: {{"user": "...", "assistant": "..."}}
"""


def extract_json_array(text: str) -> list[dict]:
    start, end = text.find("["), text.rfind("]")
    if start == -1 or end == -1:
        return []
    try:
        data = json.loads(text[start : end + 1])
    except json.JSONDecodeError:
        return []
    return [d for d in data if isinstance(d, dict)]


def one_call(client: anthropic.Anthropic, category: str) -> list[dict]:
    description, n = config.CATEGORIES[category]
    prompt = GENERATOR_PROMPT.format(
        n=n, category=category, description=description, seed=random.randint(0, 10**9)
    )
    resp = client.messages.create(
        model=config.TEACHER_MODEL,
        max_tokens=8000,
        temperature=1.0,
        messages=[{"role": "user", "content": prompt}],
    )
    text = "".join(b.text for b in resp.content if b.type == "text")
    rows = []
    for item in extract_json_array(text):
        user, assistant = item.get("user"), item.get("assistant")
        if not isinstance(user, str) or not isinstance(assistant, str):
            continue
        rows.append(
            {
                "category": category,
                "messages": [
                    {"role": "system", "content": config.SYSTEM_PROMPT},
                    {"role": "user", "content": user.strip()},
                    {"role": "assistant", "content": assistant.strip()},
                ],
            }
        )
    return rows


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--calls", type=int, default=config.CALLS_PER_CATEGORY)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()

    config.DATA_DIR.mkdir(parents=True, exist_ok=True)
    client = anthropic.Anthropic()  # reads ANTHROPIC_API_KEY
    jobs = [c for c in config.CATEGORIES for _ in range(args.calls)]
    lock = threading.Lock()
    written = 0

    with open(config.RAW_DATASET, "a", encoding="utf-8") as out, ThreadPoolExecutor(
        args.workers
    ) as pool:
        futures = [pool.submit(one_call, client, c) for c in jobs]
        for fut in tqdm(as_completed(futures), total=len(futures), desc="calls"):
            try:
                rows = fut.result()
            except anthropic.APIError as exc:
                tqdm.write(f"API error (skipped): {exc}")
                continue
            with lock:
                for row in rows:
                    out.write(json.dumps(row, ensure_ascii=False) + "\n")
                out.flush()
                written += len(rows)

    print(f"Wrote {written} raw rows to {config.RAW_DATASET}")


if __name__ == "__main__":
    main()
