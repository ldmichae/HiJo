#!/bin/bash
set -e

# Constants
BIN_NAME="nrf52840dk-sample"
TARGET_TRIPLE="thumbv7em-none-eabihf"
BIN_PATH="target/${TARGET_TRIPLE}/release/${BIN_NAME}"
BIN_OUT="nrf.bin"
PKG_OUT="pkggo.zip"
SERIAL_PORT="/dev/ttyACM0"

# Step 0: Build firmware
echo "→ Building firmware..."
cargo build --release --target "${TARGET_TRIPLE}"

# Step 1: Convert ELF to raw binary
echo "→ Creating binary from ELF..."
arm-none-eabi-objcopy -O binary "$BIN_PATH" "$BIN_OUT"

# Step 2: Generate DFU package
echo "→ Generating DFU package..."
adafruit-nrfutil dfu genpkg \
  --application "$BIN_OUT" \
  --application-version 1 \
  --dev-type 0x0052 \
  --dev-revision 0xffff \
  "$PKG_OUT"

# Step 3: Flash over serial
echo "→ Flashing over serial ($SERIAL_PORT)..."
adafruit-nrfutil dfu serial \
  --package "$PKG_OUT" \
  -p "$SERIAL_PORT" \
  --singlebank

echo "✅ Flash complete."

