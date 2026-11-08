"""Shared configuration for the LocalAutonomousAI distillation pipeline."""
from __future__ import annotations

import os
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DATA_DIR = ROOT / "data"
OUTPUT_DIR = ROOT / "output"
RAW_DATASET = DATA_DIR / "raw.jsonl"
CLEAN_DATASET = DATA_DIR / "train.jsonl"
ADAPTER_DIR = OUTPUT_DIR / "lora_adapter"
MERGED_DIR = OUTPUT_DIR / "merged_model"
GGUF_DIR = OUTPUT_DIR / "gguf"

# Teacher model (pre-production only; never shipped inside the desktop app).
TEACHER_MODEL = os.environ.get("CLAUDE_MODEL", "claude-sonnet-5-5")

# Student base model (open weights).
BASE_MODEL = os.environ.get("BASE_MODEL", "unsloth/Qwen2.5-3B-Instruct-bnb-4bit")
MAX_SEQ_LEN = 2048

SYSTEM_PROMPT = (
    "You are a direct, precise assistant. Think step by step inside "
    "<reasoning> tags, then give the final result inside <answer> tags."
)

# category -> (description, examples requested per API call)
CATEGORIES: dict[str, tuple[str, int]] = {
    "logic": ("logic puzzles, deduction and arithmetic word problems", 5),
    "coding_python": ("Python coding tasks, debugging and code explanation", 4),
    "coding_web": ("HTML, CSS, JavaScript and TypeScript/React tasks", 4),
    "instruction": ("precise instruction following with strict output formats", 5),
    "conversation": ("multi-turn style conversational repair and clarification", 5),
    "summarization": ("summarizing and rewriting short passages", 5),
    "web_grounded": (
        "answering questions using supplied web-search snippets and citing "
        "them as [1], [2]; refusing to invent facts that the snippets lack",
        4,
    ),
}

CALLS_PER_CATEGORY = int(os.environ.get("CALLS_PER_CATEGORY", "20"))
MAX_CHARS_PER_ROW = 6000
MIN_CHARS_ASSISTANT = 40
