"""Phase 2 - QLoRA fine-tuning (student learns from Claude's synthetic data) with Unsloth."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import config  # noqa: E402

TARGET_MODULES = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rank", type=int, default=32, choices=[16, 32])
    parser.add_argument("--epochs", type=int, default=2)
    parser.add_argument("--lr", type=float, default=2e-4)
    parser.add_argument("--batch", type=int, default=2)
    parser.add_argument("--accum", type=int, default=8)
    args = parser.parse_args()

    # Unsloth must be imported before transformers/trl so its patches apply.
    from unsloth import FastLanguageModel
    from unsloth.chat_templates import get_chat_template
    from datasets import load_dataset
    from trl import SFTConfig, SFTTrainer

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=config.BASE_MODEL,
        max_seq_length=config.MAX_SEQ_LEN,
        load_in_4bit=True,  # NF4 quantization
        dtype=None,
    )
    model = FastLanguageModel.get_peft_model(
        model,
        r=args.rank,
        lora_alpha=32,
        lora_dropout=0.0,
        target_modules=TARGET_MODULES,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=3407,
    )
    tokenizer = get_chat_template(tokenizer, chat_template="chatml")

    dataset = load_dataset("json", data_files=str(config.CLEAN_DATASET), split="train")

    def to_text(batch: dict) -> dict:
        return {
            "text": [
                tokenizer.apply_chat_template(m, tokenize=False, add_generation_prompt=False)
                for m in batch["messages"]
            ]
        }

    dataset = dataset.map(to_text, batched=True, remove_columns=dataset.column_names)

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=dataset,
        args=SFTConfig(
            dataset_text_field="text",
            max_seq_length=config.MAX_SEQ_LEN,
            per_device_train_batch_size=args.batch,
            gradient_accumulation_steps=args.accum,
            num_train_epochs=args.epochs,
            learning_rate=args.lr,
            lr_scheduler_type="cosine",
            warmup_ratio=0.03,
            weight_decay=0.01,
            optim="adamw_8bit",
            logging_steps=10,
            save_strategy="epoch",
            bf16=True,
            output_dir=str(config.OUTPUT_DIR / "checkpoints"),
            report_to="none",
            seed=3407,
        ),
    )
    trainer.train()

    config.ADAPTER_DIR.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(str(config.ADAPTER_DIR))
    tokenizer.save_pretrained(str(config.ADAPTER_DIR))
    print(f"Adapter saved to {config.ADAPTER_DIR}")


if __name__ == "__main__":
    main()
