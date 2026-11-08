"""Phase 2 - Merge the LoRA adapter into a standalone 16-bit model directory."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import config  # noqa: E402


def main() -> None:
    from unsloth import FastLanguageModel

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=str(config.ADAPTER_DIR),
        max_seq_length=config.MAX_SEQ_LEN,
        load_in_4bit=True,
    )
    config.MERGED_DIR.mkdir(parents=True, exist_ok=True)
    # "merged_16bit" dequantizes and fuses the adapter into fp16 weights.
    model.save_pretrained_merged(str(config.MERGED_DIR), tokenizer, save_method="merged_16bit")
    print(f"Merged fp16 model written to {config.MERGED_DIR}")


if __name__ == "__main__":
    main()
