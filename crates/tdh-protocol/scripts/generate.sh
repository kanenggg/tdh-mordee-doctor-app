#!/bin/bash
set -e

# Generate code using protoc directly

PROTO_DIR="protos"
SCALA_OUT_DIR="scala/src/generated"
RUST_OUT_DIR="rust/src"
PYTHON_OUT_DIR="python/tdh_protocol"
TS_OUT_DIR="typescript/src"

# Create output directories
mkdir -p "$SCALA_OUT_DIR" "$RUST_OUT_DIR" "$PYTHON_OUT_DIR" "$TS_OUT_DIR"

# TypeScript (already has @protobuf-ts/plugin installed)
echo "Generating TypeScript..."
protoc \
  --proto_path="$PROTO_DIR" \
  --ts_out="$TS_OUT_DIR" \
  $(find "$PROTO_DIR" -name "*.proto")

echo "Code generation complete!"
