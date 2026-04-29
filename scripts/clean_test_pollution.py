"""One-shot cleanup for test fixture pollution in ~/.pour/cache/history.jsonl.

Background: integration tests in `tests/server_*.rs` previously called the
submit handler without setting `POUR_HOME`, so `History::load()` fell through
to the user's real `~/.pour/cache/history.jsonl`. This script removes those
polluted entries while preserving real captures.

Usage:
    python scripts/clean_test_pollution.py --dry-run   # preview
    python scripts/clean_test_pollution.py --apply     # rewrite the file

The original file is backed up (caller's responsibility — see EnvGuard guidance
in `pour - docs/08 specs/pour-test-isolation.md`).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

# Test-fixture vault_paths produced by integration tests. Real captures never
# write to these paths because they live under the user's Obsidian vault root,
# never the bare `Coffee/`, `Journal/`, `Form/`, or `test/` namespaces.
TEST_VAULT_PATHS_EXACT = {
    "Coffee/note.md",
    "Coffee/note1.md",
    "Coffee/note2.md",
    "Coffee/2026/test.md",
    "Journal/daily.md",
    "Journal/test.md",
    "Form/note.md",
    "test.md",
    "test1.md",
    "test2.md",
    "test3.md",
}
TEST_VAULT_PATH_PREFIXES = ("test/", "beans/")  # data_history fixtures, autocreate fixtures
TEST_VAULT_PATH_REGEX = re.compile(r"^(Beans/.+\.md|Coffee/test.*\.md)$")


def is_test_path(vault_path: str) -> bool:
    if vault_path in TEST_VAULT_PATHS_EXACT:
        return True
    if any(vault_path.startswith(p) for p in TEST_VAULT_PATH_PREFIXES):
        return True
    if TEST_VAULT_PATH_REGEX.match(vault_path):
        return True
    return False


def split_concatenated(line: str) -> list[str]:
    """Some appender races wrote two JSONL entries on one line with no newline
    between them (e.g. `}{`). Split those back apart so we can classify each."""
    if "}{" not in line:
        return [line] if line.strip() else []
    parts: list[str] = []
    depth = 0
    start = 0
    for i, ch in enumerate(line):
        if ch == "{":
            if depth == 0:
                start = i
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                parts.append(line[start : i + 1])
    return parts


def classify(parts: list[str]) -> tuple[list[str], list[str]]:
    keep: list[str] = []
    drop: list[str] = []
    for p in parts:
        try:
            obj = json.loads(p)
        except json.JSONDecodeError:
            drop.append(p)
            continue
        vault_path = obj.get("vault_path", "")
        if is_test_path(vault_path):
            drop.append(p)
        else:
            keep.append(p)
    return keep, drop


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true", help="rewrite the file in place")
    parser.add_argument("--dry-run", action="store_true", help="preview only (default)")
    parser.add_argument(
        "--path",
        default=os.environ.get("POUR_HISTORY_PATH"),
        help="path to history.jsonl (defaults to ~/.pour/cache/history.jsonl)",
    )
    args = parser.parse_args()

    if args.apply and args.dry_run:
        print("--apply and --dry-run are mutually exclusive", file=sys.stderr)
        return 2

    path = Path(args.path) if args.path else Path.home() / ".pour" / "cache" / "history.jsonl"
    if not path.exists():
        print(f"history file not found: {path}", file=sys.stderr)
        return 1

    raw = path.read_text(encoding="utf-8").splitlines()
    kept_lines: list[str] = []
    dropped_lines: list[str] = []
    blank_lines = 0

    for line in raw:
        if not line.strip():
            blank_lines += 1
            continue
        parts = split_concatenated(line)
        keep, drop = classify(parts)
        kept_lines.extend(keep)
        dropped_lines.extend(drop)

    print(f"=== history.jsonl: {path} ===")
    print(f"  raw lines:        {len(raw)}")
    print(f"  blank lines:      {blank_lines}  (always dropped)")
    print(f"  kept entries:     {len(kept_lines)}")
    print(f"  dropped entries:  {len(dropped_lines)}")
    print()

    if kept_lines:
        print("--- KEPT (preview, up to 5) ---")
        for line in kept_lines[:5]:
            print(f"  {line}")
        if len(kept_lines) > 5:
            print(f"  ... and {len(kept_lines) - 5} more")
        print()

    if dropped_lines:
        print("--- DROPPED (preview, up to 10) ---")
        for line in dropped_lines[:10]:
            print(f"  {line}")
        if len(dropped_lines) > 10:
            print(f"  ... and {len(dropped_lines) - 10} more")
        print()

    if not args.apply:
        print("dry run — no changes written. Re-run with --apply to rewrite the file.")
        return 0

    out = "\n".join(kept_lines)
    if out:
        out += "\n"
    path.write_text(out, encoding="utf-8")
    print(f"WROTE {len(kept_lines)} entries to {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
