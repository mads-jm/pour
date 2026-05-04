#!/usr/bin/env bash
# check-file-size.sh — CI ratchet: fail if any src/**/*.rs exceeds 800 lines
# without an explicit "// LINTOK: oversized:" annotation.
#
# Usage: bash scripts/check-file-size.sh
# Exit code: 0 = clean, 1 = one or more unannotated violations.

set -euo pipefail

LIMIT=800
ANNOTATION="// LINTOK: oversized:"
SRC_DIR="$(dirname "$0")/../src"

violations=0
checked=0

while IFS= read -r -d '' file; do
    lines=$(wc -l < "$file")
    checked=$((checked + 1))

    if [ "$lines" -gt "$LIMIT" ]; then
        if grep -qF "$ANNOTATION" "$file"; then
            echo "  OK  ($lines lines, annotated): $file"
        else
            echo "FAIL  ($lines lines, no annotation): $file"
            violations=$((violations + 1))
        fi
    fi
done < <(find "$SRC_DIR" -name "*.rs" -print0)

echo ""
echo "Checked $checked file(s). Limit: $LIMIT lines."

if [ "$violations" -gt 0 ]; then
    echo "FAILED: $violations file(s) exceed $LIMIT lines without a '${ANNOTATION}' comment."
    echo "Add the annotation near the top of the file, e.g.:"
    echo "  // LINTOK: oversized: pending decomposition"
    exit 1
else
    echo "All files within budget or annotated. OK."
    exit 0
fi
