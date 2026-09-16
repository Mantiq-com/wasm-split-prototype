#!/usr/bin/env bash
# Builds the sparse_shares test artefact, splits it with the wasm_split_cli
# binary and summarises the output: how many chunks were produced for how many
# splits, how many chunks carry no code or data at all, and how large the
# generated link module is. Fails when a chunk defines nothing or when the
# counts differ from what build.rs describes.
#
#   SPARSE_SHARES_SPLITS=40 ./repro.sh
#
# Requires `wasm-objdump` (wabt) to classify empty chunks.
set -euo pipefail

THIS_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
INTEGRATION_DIR=$THIS_DIR/..
ROOT_DIR=$INTEGRATION_DIR/..
OUT_DIR=$INTEGRATION_DIR/target/sparse_shares_split
N=${SPARSE_SHARES_SPLITS:-12}

command -v wasm-objdump >/dev/null || {
    echo >&2 "wasm-objdump not found (install wabt)"
    exit 1
}

file_size() {
    wc -c <"$1" | tr -d ' '
}

echo "== building sparse_shares (SPARSE_SHARES_SPLITS=$N)"
wasm=$(cd -- "$INTEGRATION_DIR" && SPARSE_SHARES_SPLITS=$N cargo test --no-run --target wasm32-unknown-unknown -p sparse_shares 2>&1 |
    tee /dev/stderr |
    sed -n 's/.*Executable .*(\(.*\.wasm\)).*/\1/p' |
    tail -n 1)
[ -n "$wasm" ] || {
    echo >&2 "could not find the built test artefact"
    exit 1
}
# cargo prints the path relative to the workspace root
case $wasm in
/*) ;;
*) wasm=$INTEGRATION_DIR/$wasm ;;
esac

echo "== splitting $wasm"
rm -rf -- "$OUT_DIR"
mkdir -p -- "$OUT_DIR"
(cd -- "$ROOT_DIR" && cargo run --quiet --release -p wasm_split_cli_support --features build-binary -- "$wasm" "$OUT_DIR")

splits=0
chunks=0
empty_chunks=0
empty_bytes=0
for f in "$OUT_DIR"/split_*.wasm; do
    [ -e "$f" ] && splits=$((splits + 1))
done
for f in "$OUT_DIR"/chunk_*.wasm; do
    [ -e "$f" ] || continue
    chunks=$((chunks + 1))
    # A chunk that defines no function, table entry, export or global and
    # whose data segments are all zero-length contributes nothing when it is
    # instantiated; its only purpose is to be fetched.
    if ! wasm-objdump --headers -- "$f" |
        awk '/^ +(Function|Table|Memory|Global|Export|Elem|Tag) .*count: [1-9]/ { found = 1 } END { exit !found }' &&
        ! { wasm-objdump --details --section=Data -- "$f" 2>/dev/null || true; } |
        awk '/segment\[[0-9]+\].*size=[1-9]/ { found = 1 } END { exit !found }'; then
        empty_chunks=$((empty_chunks + 1))
        empty_bytes=$((empty_bytes + $(file_size "$f")))
    fi
done

link_js=$OUT_DIR/__wasm_split.js
link_size=$(file_size "$link_js")
link_chunk_loaders=$(awk '/^const __chunk_/ { n++ } END { print n + 0 }' "$link_js")
expected_chunks=$((2 * N - 2))

echo "== result in $OUT_DIR"
printf '%-32s %8d\n' "split modules" "$splits"
printf '%-32s %8d\n' "chunk modules" "$chunks"
printf '%-32s %8d\n' "  chunks with no code or data" "$empty_chunks"
printf '%-32s %8d\n' "  bytes in those chunks" "$empty_bytes"
printf '%-32s %8d\n' "link module bytes" "$link_size"
printf '%-32s %8d\n' "chunk loaders in link module" "$link_chunk_loaders"

status=0
if [ "$empty_chunks" -gt 0 ]; then
    echo "FAIL: $empty_chunks chunk modules are fetched but define nothing"
    status=1
fi
if [ "$splits" -ne "$N" ] || [ "$chunks" -ne "$expected_chunks" ]; then
    echo "FAIL: expected $N split and $expected_chunks chunk modules, see build.rs"
    status=1
fi
exit "$status"
