#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
INSTALL_DIR="${HOME}/.local/bin"
BINARY_NAME="ghwatch"

cd "${SCRIPT_DIR}"

echo "Building ${BINARY_NAME} in release mode..."
cargo build --release

mkdir -p "${INSTALL_DIR}"

echo "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."
install -m 755 "target/release/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"

echo "Installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"

case ":${PATH}:" in
    *:"${INSTALL_DIR}":*) ;;
    *) echo "Warning: ${INSTALL_DIR} is not in your PATH. Add it to use '${BINARY_NAME}' directly." ;;
esac
