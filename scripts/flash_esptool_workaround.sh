#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "usage: $0 <elf-file>"
  exit 1
fi

ELF_PATH="$1"
ELF_DIR="$(dirname "$ELF_PATH")"
BUILD_ROOT="${ELF_DIR}/build"
BIN_PATH="${ELF_PATH}.bin"
ESP_PORT="${ESP_PORT:-/dev/ttyACM0}"
MONITOR_MODE="${MONITOR_MODE:-raw}"

detect_fallback_port() {
  local candidate
  for candidate in /dev/ttyACM* /dev/ttyUSB*; do
    if [ -e "$candidate" ]; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

wait_for_port() {
  local port="$1"
  local timeout_s="${2:-10}"
  local i
  for ((i=0; i<timeout_s*10; i++)); do
    if [ -e "$port" ]; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

FLASH_ARGS_FILE="$(find "$BUILD_ROOT" -type f -path '*/esp-idf-sys-*/out/build/flash_args' -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -n1 | cut -d' ' -f2-)"

if [ -z "${FLASH_ARGS_FILE}" ]; then
  echo "Could not find flash_args under: ${BUILD_ROOT}"
  echo "Run a build first: cargo +esp build"
  exit 1
fi

FLASH_DIR="$(dirname "$FLASH_ARGS_FILE")"

if command -v esptool >/dev/null 2>&1; then
  ESPTOOL_CMD=(esptool)
elif command -v esptools >/dev/null 2>&1; then
  ESPTOOL_CMD=(esptools tool)
elif [ -x "$HOME/.cargo/bin/esptools" ]; then
  ESPTOOL_CMD=("$HOME/.cargo/bin/esptools" tool)
else
  echo "esptool/esptools not found. Install with: cargo install esptools"
  exit 127
fi

if command -v espflash >/dev/null 2>&1; then
  ESPFLASH_CMD=(espflash)
elif [ -x "$HOME/.cargo/bin/espflash" ]; then
  ESPFLASH_CMD=("$HOME/.cargo/bin/espflash")
else
  echo "espflash not found. Install with: cargo install espflash"
  exit 127
fi

echo "[workaround] Flashing using IDF-generated flash_args: ${FLASH_ARGS_FILE}"

read -r FLASH_OPTS_LINE < "${FLASH_ARGS_FILE}"
read -r -a FLASH_OPTS <<< "${FLASH_OPTS_LINE}"

for i in "${!FLASH_OPTS[@]}"; do
  case "${FLASH_OPTS[$i]}" in
    --flash_mode) FLASH_OPTS[$i]=--flash-mode ;;
    --flash_freq) FLASH_OPTS[$i]=--flash-freq ;;
    --flash_size) FLASH_OPTS[$i]=--flash-size ;;
  esac
done

echo "[workaround] Converting ELF to BIN via esptool elf2image"
"${ESPTOOL_CMD[@]}" --chip esp32p4 elf2image --output "${BIN_PATH}" "${FLASH_OPTS[@]}" "${ELF_PATH}"

first_write=1
while read -r address image_rel; do
  if [ -z "${address}" ] || [ -z "${image_rel}" ]; then
    continue
  fi

  image_path="${FLASH_DIR}/${image_rel}"
  if [ "${image_rel}" = "libespidf.bin" ]; then
    image_path="${BIN_PATH}"
  fi

  if [ ! -f "${image_path}" ]; then
    echo "Missing image from flash_args: ${image_path}"
    exit 1
  fi

  echo "[workaround] write-bin ${address} ${image_path}"
  if [ "${first_write}" -eq 1 ]; then
    "${ESPFLASH_CMD[@]}" write-bin --chip esp32p4 --port "${ESP_PORT}" --no-stub --after no-reset "${address}" "${image_path}"
    first_write=0
  else
    "${ESPFLASH_CMD[@]}" write-bin --chip esp32p4 --port "${ESP_PORT}" --no-stub --before no-reset --after no-reset "${address}" "${image_path}"
  fi
done < <(grep -E '^0x[0-9a-fA-F]+' "${FLASH_ARGS_FILE}")

echo "[workaround] Resetting chip to start flashed app"
"${ESPFLASH_CMD[@]}" reset --chip esp32p4 --port "${ESP_PORT}" --no-stub --after hard-reset

MONITOR_PORT="${ESP_PORT}"
if ! wait_for_port "${MONITOR_PORT}" 10; then
  if FALLBACK_PORT="$(detect_fallback_port)"; then
    echo "[workaround] Port ${MONITOR_PORT} not present after reset, falling back to ${FALLBACK_PORT}"
    MONITOR_PORT="${FALLBACK_PORT}"
  else
    echo "[workaround] Port ${MONITOR_PORT} not present after reset and no fallback port found"
    exit 1
  fi
fi

case "${MONITOR_MODE}" in
  raw)
    echo "[workaround] Opening raw serial monitor on ${MONITOR_PORT} @ 115200 (Ctrl+C to exit)"
    stty -F "${MONITOR_PORT}" 115200 cs8 -cstopb -parenb -ixon -ixoff -echo -echoe -echok -echoctl -echoke -icanon -isig -iexten || true
    cat "${MONITOR_PORT}"
    ;;
  espflash)
    echo "[workaround] Opening espflash monitor on ${MONITOR_PORT} (no reset/no sync)"
    "${ESPFLASH_CMD[@]}" monitor --chip esp32p4 --port "${MONITOR_PORT}" --no-stub --before no-reset-no-sync --after no-reset
    ;;
  *)
    echo "Invalid MONITOR_MODE='${MONITOR_MODE}'. Use 'raw' or 'espflash'."
    exit 2
    ;;
esac
