#!/usr/bin/env bash
# Compare oa-forge against other OpenAPI code generators using hyperfine.
#
# Prerequisites:
#   - cargo install --path crates/cli  (or cargo build --release)
#   - npm install -g orval @hey-api/openapi-ts openapi-typescript
#   - brew install hyperfine (or cargo install hyperfine)
#
# Usage:
#   ./scripts/bench-compare.sh [spec_file]
#
# Default spec: tests/fixtures/petstore.yaml

set -euo pipefail

SPEC="${1:-tests/fixtures/petstore.yaml}"
OUT_DIR=$(mktemp -d)
trap 'rm -rf "$OUT_DIR"' EXIT

if ! command -v hyperfine &>/dev/null; then
  echo "Error: hyperfine not found. Install via: brew install hyperfine" >&2
  exit 1
fi

echo "Benchmarking with spec: $SPEC"
echo "Output directory: $OUT_DIR"
echo

# Build oa-forge in release mode
cargo build --release --quiet 2>/dev/null

CMDS=()
NAMES=()

# oa-forge (always available)
CMDS+=("./target/release/oa-forge generate --input $SPEC --output $OUT_DIR/oa-forge --hooks")
NAMES+=("oa-forge")

# Orval (if available)
if command -v orval &>/dev/null; then
  # Create a minimal orval config
  cat > "$OUT_DIR/orval.config.cjs" <<EOF
module.exports = {
  petstore: {
    input: '$(realpath "$SPEC")',
    output: { target: '$OUT_DIR/orval-out/api.ts' },
  },
};
EOF
  CMDS+=("orval --config $OUT_DIR/orval.config.cjs")
  NAMES+=("orval")
else
  echo "Note: orval not found, skipping"
fi

# @hey-api/openapi-ts (if available)
if command -v openapi-ts &>/dev/null; then
  CMDS+=("openapi-ts -i $SPEC -o $OUT_DIR/hey-api-out")
  NAMES+=("@hey-api/openapi-ts")
else
  echo "Note: @hey-api/openapi-ts not found, skipping"
fi

# openapi-typescript (if available)
if command -v openapi-typescript &>/dev/null; then
  CMDS+=("openapi-typescript $SPEC -o $OUT_DIR/openapi-ts-out/types.ts")
  NAMES+=("openapi-typescript")
else
  echo "Note: openapi-typescript not found, skipping"
fi

echo
echo "=== Running benchmark ==="
echo

# Build hyperfine arguments
HYPERFINE_ARGS=("--warmup" "3" "--min-runs" "10" "--export-markdown" "$OUT_DIR/results.md")
for i in "${!CMDS[@]}"; do
  HYPERFINE_ARGS+=("--command-name" "${NAMES[$i]}" "${CMDS[$i]}")
done

hyperfine "${HYPERFINE_ARGS[@]}"

echo
echo "=== Results ==="
cat "$OUT_DIR/results.md"
