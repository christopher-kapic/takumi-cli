#!/bin/sh
set -e

INSTALL_DIR="$HOME/.local/bin"
BIN_NAME="takumi"

# Ensure install directory exists
mkdir -p "$INSTALL_DIR"

echo "Building takumi-cli (release)..."
cargo build -p takumi-cli --release

cp "target/release/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
echo "Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"

# Check if install dir is in PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    echo ""
    echo "Note: $INSTALL_DIR is not in your PATH."
    echo "Add it by appending this to your shell profile:"
    echo ""
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    ;;
esac
