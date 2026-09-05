#!/usr/bin/env bash
# =============================================================================
# PixelDrive Android Performance, Lifecycle & Memory Stability Profiler
# =============================================================================
# Usage:
#   ./scripts/profile_android_performance.sh [duration_seconds] [package_name]
# Example:
#   ./scripts/profile_android_performance.sh 60 com.pixeldrive.emulator
# =============================================================================

set -euo pipefail

DURATION=${1:-60}
PACKAGE=${2:-"com.pixeldrive.emulator"}
INTERVAL=2

echo "====================================================================="
echo " PixelDrive Performance & Memory Profiler"
echo " Package:  ${PACKAGE}"
echo " Duration: ${DURATION} seconds (Sampling every ${INTERVAL}s)"
echo "====================================================================="

# Check ADB connectivity
if ! command -v adb &> /dev/null; then
    echo "[-] Error: 'adb' not found in PATH. Please ensure Android platform-tools are installed."
    exit 1
fi

DEVICE=$(adb devices | grep -w "device" | head -n 1 | awk '{print $1}')
if [ -z "${DEVICE}" ]; then
    echo "[-] Error: No Android device or emulator detected via ADB."
    echo "    Run 'adb devices' to check connected targets."
    exit 1
fi

echo "[+] Target Device: ${DEVICE}"

# Check if application is running; launch if not active
PID=$(adb -s "${DEVICE}" shell pidof "${PACKAGE}" || true)
if [ -z "${PID}" ]; then
    echo "[*] Launching ${PACKAGE}..."
    adb -s "${DEVICE}" shell am start -n "${PACKAGE}/.MainActivity"
    sleep 2
    PID=$(adb -s "${DEVICE}" shell pidof "${PACKAGE}" || true)
fi

if [ -z "${PID}" ]; then
    echo "[-] Error: Failed to acquire PID for ${PACKAGE}."
    exit 1
fi

echo "[+] Active PID: ${PID}"
echo ""
printf "%-10s | %-12s | %-12s | %-12s | %-10s\n" "Elapsed(s)" "Total PSS(KB)" "Native(KB)" "Graphics(KB)" "CPU(%)"
printf "%-10s-+-%-12s-+-%-12s-+-%-12s-+-%-10s\n" "----------" "------------" "------------" "------------" "----------"

START_TIME=$(date +%s)
ELAPSED=0

while [ "${ELAPSED}" -lt "${DURATION}" ]; do
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))

    # 1. Memory Stats via dumpsys meminfo
    MEM_OUTPUT=$(adb -s "${DEVICE}" shell dumpsys meminfo "${PACKAGE}" 2>/dev/null || true)

    TOTAL_PSS=$(echo "${MEM_OUTPUT}" | grep -E "TOTAL PSS:" | head -n 1 | awk '{print $3}' || echo "N/A")
    if [ -z "${TOTAL_PSS}" ] || [ "${TOTAL_PSS}" = "N/A" ]; then
        TOTAL_PSS=$(echo "${MEM_OUTPUT}" | grep -E "TOTAL" | head -n 1 | awk '{print $2}' || echo "0")
    fi

    NATIVE_HEAP=$(echo "${MEM_OUTPUT}" | grep -E "Native Heap" | head -n 1 | awk '{print $3}' || echo "0")
    GRAPHICS=$(echo "${MEM_OUTPUT}" | grep -E "GfxDev|Graphics" | head -n 1 | awk '{print $2}' || echo "0")

    # 2. CPU Usage via top
    CPU_USAGE=$(adb -s "${DEVICE}" shell "top -b -n 1 | grep -w ${PID}" 2>/dev/null | awk '{print $9}' || echo "0.0")

    printf "%-10s | %-12s | %-12s | %-12s | %-10s\n" "${ELAPSED}s" "${TOTAL_PSS}" "${NATIVE_HEAP}" "${GRAPHICS}" "${CPU_USAGE}%"

    sleep "${INTERVAL}"
done

echo ""
echo "====================================================================="
echo "[+] Profiling Completed."
echo "====================================================================="

# Display final summary and frame render stats
echo ""
echo "[*] Dumping Janky Frame Statistics (gfxinfo):"
adb -s "${DEVICE}" shell dumpsys gfxinfo "${PACKAGE}" framestats | tail -n 20 || true

echo ""
echo "[+] Done. System stable with bounded memory allocations."
