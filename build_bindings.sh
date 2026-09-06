#!/bin/bash
#
# Build the crrdb-mcp UniFFI surface: generate Swift bindings and compile
# static libraries for iOS. Mirrors atman's build_bindings.sh.
#
# Usage:
#   ./build_bindings.sh --sim-arm64            # Apple Silicon simulator
#   ./build_bindings.sh --arm64 --release      # device, release profile

set -uexo pipefail

PROJECT_NAME="crrdb_mcp"
TARGET_DIR="target"

BUILD_MODE="debug"
BUILD_ARM64=false
BUILD_SIM_ARM64=false
BUILD_X86_64=false

while (( $# )); do
    case "$1" in
        --release)   BUILD_MODE="release" ;;
        --arm64)     BUILD_ARM64=true ;;
        --sim-arm64) BUILD_SIM_ARM64=true ;;
        --x86_64)    BUILD_X86_64=true ;;
        *)
            echo "build_bindings.sh: unknown argument: $1" >&2
            exit 2
            ;;
    esac
    shift
done

CARGO_FLAGS=""
if [[ "$BUILD_MODE" == "release" ]]; then
    CARGO_FLAGS="--release"
fi

if ! $BUILD_ARM64 && ! $BUILD_SIM_ARM64 && ! $BUILD_X86_64; then
    echo "no target specified!"
    exit 1
fi

SWIFT_OUT="${TARGET_DIR}/uniffi-bindings/swift"
mkdir -p "${SWIFT_OUT}"
# uniffi-bindgen reads metadata from the host cdylib (not the iOS .a),
# so we build it separately. Use `--crate-type cdylib` because the manifest
# only emits staticlib — emitting cdylib unconditionally breaks iOS linking.
cargo rustc $CARGO_FLAGS --features ffi --lib --crate-type cdylib
HOST_LIB_EXT="dylib"
[[ "$(uname)" == "Linux" ]] && HOST_LIB_EXT="so"
HOST_LIB="${TARGET_DIR}/${BUILD_MODE}/lib${PROJECT_NAME}.${HOST_LIB_EXT}"
cargo run --features bindgen --bin uniffi-bindgen -- generate \
    --library "${HOST_LIB}" \
    --language swift \
    --out-dir "${SWIFT_OUT}"

LIB_NAME="lib${PROJECT_NAME}.a"

if $BUILD_ARM64; then
    cargo build --lib --target aarch64-apple-ios $CARGO_FLAGS --features ffi
    lipo -info ${TARGET_DIR}/aarch64-apple-ios/${BUILD_MODE}/${LIB_NAME}
fi
if $BUILD_SIM_ARM64; then
    cargo build --lib --target aarch64-apple-ios-sim $CARGO_FLAGS --features ffi
    lipo -info ${TARGET_DIR}/aarch64-apple-ios-sim/${BUILD_MODE}/${LIB_NAME}
fi
if $BUILD_X86_64; then
    cargo build --lib --target x86_64-apple-ios $CARGO_FLAGS --features ffi
    lipo -info ${TARGET_DIR}/x86_64-apple-ios/${BUILD_MODE}/${LIB_NAME}
fi
