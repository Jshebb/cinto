#!/usr/bin/env python3
import argparse
import glob
import json
import os
import sys
from collections import Counter


def iter_trace_paths(inputs):
    for item in inputs:
        if os.path.isdir(item):
            yield from sorted(glob.glob(os.path.join(item, "*.jsonl")))
        else:
            yield from sorted(glob.glob(item))


def load_records(paths):
    for path in paths:
        with open(path, "r", encoding="utf-8") as handle:
            for line_number, line in enumerate(handle, start=1):
                line = line.strip()
                if not line:
                    continue
                try:
                    record = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise SystemExit(f"{path}:{line_number}: invalid JSON: {exc}") from exc
                yield path, record


def usable(record, require_crp_valid):
    if not record.get("workflow_succeeded", False):
        return False
    if record.get("error"):
        return False
    if require_crp_valid and not record.get("crp_valid", False):
        return False
    return all(record.get(field) for field in ("system_prompt", "user_message", "model_response"))


def training_example(record):
    return {
        "messages": [
            {"role": "system", "content": record["system_prompt"]},
            {"role": "user", "content": record["user_message"]},
            {"role": "assistant", "content": record["model_response"]},
        ]
    }


def main():
    parser = argparse.ArgumentParser(
        description="Convert kernel trace JSONL files into chat fine-tuning JSONL."
    )
    parser.add_argument(
        "inputs",
        nargs="+",
        help="Trace JSONL files, globs, or directories containing trace JSONL files.",
    )
    parser.add_argument(
        "-o",
        "--output",
        required=True,
        help="Output training JSONL path.",
    )
    parser.add_argument(
        "--require-crp-valid",
        action="store_true",
        help="Keep only stage traces whose response parsed as valid CRP.",
    )
    args = parser.parse_args()

    paths = list(dict.fromkeys(iter_trace_paths(args.inputs)))
    if not paths:
        raise SystemExit("no trace JSONL files matched")

    os.makedirs(os.path.dirname(args.output) or ".", exist_ok=True)

    counts = Counter()
    by_stage = Counter()
    with open(args.output, "w", encoding="utf-8") as out:
        for _, record in load_records(paths):
            counts["seen"] += 1
            if not usable(record, args.require_crp_valid):
                counts["skipped"] += 1
                continue
            out.write(json.dumps(training_example(record), ensure_ascii=False) + "\n")
            counts["written"] += 1
            by_stage[record.get("stage", "unknown")] += 1

    print(f"input_files={len(paths)} seen={counts['seen']} written={counts['written']} skipped={counts['skipped']}")
    if by_stage:
        print("by_stage=" + ", ".join(f"{stage}:{count}" for stage, count in sorted(by_stage.items())))

    return 0


if __name__ == "__main__":
    sys.exit(main())
