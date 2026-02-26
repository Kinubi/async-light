#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_BASE="${ROOT_DIR}/target/riscv32imafc-esp-espidf/debug/build"
DRY_RUN=0

if [ "${1:-}" = "--dry-run" ]; then
  DRY_RUN=1
fi

if [ ! -d "${TARGET_BASE}" ]; then
  echo "Build directory not found: ${TARGET_BASE}"
  echo "Run 'cargo build' first."
  exit 1
fi

OUT_DIR="$(find "${TARGET_BASE}" -maxdepth 2 -type d -path '*/esp-idf-sys-*/out' -printf '%T@ %p\n' | sort -nr | head -n1 | cut -d' ' -f2-)"

if [ -z "${OUT_DIR}" ] || [ ! -f "${OUT_DIR}/esp-idf-build.json" ]; then
  echo "Could not find generated esp-idf out directory."
  echo "Run 'cargo build' first."
  exit 1
fi

readarray -t BUILD_INFO < <(python3 - <<'PY' "${OUT_DIR}/esp-idf-build.json"
import json
import sys
from pathlib import Path
import re

meta = json.loads(Path(sys.argv[1]).read_text())
print(meta["esp_idf_dir"])
print(meta["exported_path_var"])
print(meta["venv_python"])

idf_dir = meta["esp_idf_dir"]
match = re.search(r"v?(\d+)\.(\d+)(?:\.\d+)?$", idf_dir)
if match:
  print(f"{match.group(1)}.{match.group(2)}")
else:
  print("5.5")
PY
)

IDF_PATH="${BUILD_INFO[0]}"
EXPORTED_PATH="${BUILD_INFO[1]}"
VENV_PYTHON="${BUILD_INFO[2]}"
ESP_IDF_VERSION_MM="${BUILD_INFO[3]}"
SDKCONFIG_DEFAULTS="${OUT_DIR}/gen-sdkconfig.defaults;${ROOT_DIR}/sdkconfig.defaults"

if [ ! -f "${ROOT_DIR}/sdkconfig.defaults" ]; then
  echo "Missing ${ROOT_DIR}/sdkconfig.defaults"
  exit 1
fi

if [ ! -f "${OUT_DIR}/gen-sdkconfig.defaults" ]; then
  echo "Missing ${OUT_DIR}/gen-sdkconfig.defaults"
  echo "Run 'cargo build' once to regenerate it."
  exit 1
fi

echo "Using out dir: ${OUT_DIR}"
echo "Regenerating sdkconfig from defaults, then opening menuconfig..."

export IDF_PATH
export IDF_TARGET=esp32p4
export PROJECT_DIR="${ROOT_DIR}"
export PATH="${EXPORTED_PATH}"
export SDKCONFIG_DEFAULTS
export ESP_IDF_VERSION="${ESP_IDF_VERSION_MM}"

cd "${OUT_DIR}"
rm -f sdkconfig
cmake -S . -B build -G Ninja

"${VENV_PYTHON}" -m kconfgen \
  --list-separator=semicolon \
  --kconfig "${IDF_PATH}/Kconfig" \
  --sdkconfig-rename "${IDF_PATH}/sdkconfig.rename" \
  --config "${OUT_DIR}/sdkconfig" \
  --env IDF_MINIMAL_BUILD=n \
  --env-file "${OUT_DIR}/build/config.env" \
  --dont-write-deprecated \
  --output config "${OUT_DIR}/sdkconfig"

if [ "${DRY_RUN}" -eq 1 ]; then
  echo "Dry run complete: sdkconfig regenerated from defaults."
else
  cmake --build build --target menuconfig
fi
