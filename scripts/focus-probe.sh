#!/usr/bin/env bash
# scripts/focus-probe.sh
#
# 用途：自动化度量 PokePoke popup 是否抢用户的输入焦点。
#
# 判定方式：
#   1. 读当前 frontmost 应用的 bundle id 作为 baseline
#      （典型场景：在 Cursor / iTerm / VSCode 里敲命令跑这个脚本，
#        baseline 就是你自己的编辑器/终端）
#   2. 触发一次 POST /notify，强制 PokePoke 弹 popup
#   3. 等 500ms 让 popup 真正显示出来
#   4. 再读一次 frontmost bundle id
#      - 若仍为 baseline → 没抢焦点（期望）
#      - 若变成 com.pokepoke.app 或其他 → 抢焦点了（失败）
#   5. 重复 N 次（默认 20），输出 "stolen_count / total"
#
# 前置：
#   - PokePoke 必须已在运行（HTTP 服务监听 9876）
#   - baseline 不能是 PokePoke 本身——请在跑脚本前把鼠标/焦点
#     放在其他应用（比如 Cursor / iTerm2）的窗口里
#   - osascript 读 frontmost 需要一次性授权（首次运行会弹系统授权框）
#
# 使用：
#   ./scripts/focus-probe.sh           # 默认 N=20
#   N=5 ./scripts/focus-probe.sh       # 自定义次数

set -euo pipefail

PORT="${PORT:-9876}"
N="${N:-20}"
NOTIFY_URL="http://127.0.0.1:${PORT}/notify"
INTER_TEST_DELAY_MS=800   # 每轮之间的间隔，给 PokePoke 缓冲
POST_POPUP_WAIT_MS=500    # popup 触发后等多久再读 frontmost

# 工具函数 ----------------------------------------------------------------

die() {
  echo "ERROR: $*" >&2
  exit 1
}

frontmost_bundle_id() {
  # 读当前 frontmost 应用的 bundle identifier
  osascript -e 'tell application "System Events" to get bundle identifier of first application process whose frontmost is true' 2>/dev/null
}

trigger_popup() {
  local id="probe-$(date +%s%N)"
  curl -s --max-time 3 -X POST "$NOTIFY_URL" \
    -H "Content-Type: application/json" \
    -d "{\"task_id\":\"$id\",\"title\":\"Focus Probe\",\"message\":\"probe $id\",\"source\":\"focus-probe\",\"status\":\"success\"}" \
    >/dev/null
}

sleep_ms() {
  # macOS 的 sleep 支持小数秒
  local ms="$1"
  local sec
  sec=$(awk "BEGIN { printf \"%.3f\", $ms / 1000 }")
  sleep "$sec"
}

# 前置检查 ----------------------------------------------------------------

if ! command -v curl >/dev/null 2>&1; then
  die "curl not found"
fi

if ! curl -s --max-time 2 -o /dev/null -w "%{http_code}" "$NOTIFY_URL" \
  -X POST -H "Content-Type: application/json" -d '{}' | grep -qE "^(2|4)"; then
  die "PokePoke HTTP server not responding at $NOTIFY_URL. Start it via 'npm run tauri dev' first."
fi

# 主流程 ------------------------------------------------------------------

echo "[focus-probe] 3 秒后开始测量；请在此之前把焦点放到你要保护的应用（Cursor / iTerm / 编辑器）..."
sleep_ms 3000

baseline="$(frontmost_bundle_id)"
if [[ -z "$baseline" ]]; then
  die "无法读取 frontmost bundle id（osascript 权限未授予？）"
fi
echo "[focus-probe] baseline frontmost = $baseline"

if [[ "$baseline" == *"pokepoke"* ]]; then
  die "baseline 已经是 PokePoke，测试无意义；请把焦点切到其他应用再重跑。"
fi

stolen=0
observed_after=()

for ((i = 1; i <= N; i++)); do
  # 每轮开始前不再主动切换前景应用（避免 AppleEvent 权限问题）。
  # 如果上一轮被抢焦点还没切回来，直接记录下一轮继续测——多轮连续
  # 抢焦点能反映真实用户感受（连续 popup 打断）。
  trigger_popup
  sleep_ms "$POST_POPUP_WAIT_MS"

  after="$(frontmost_bundle_id)"
  observed_after+=("$after")

  if [[ "$after" != "$baseline" ]]; then
    stolen=$((stolen + 1))
    echo "  [$i/$N] STOLEN: frontmost became '$after'"
  else
    echo "  [$i/$N] ok"
  fi

  sleep_ms "$INTER_TEST_DELAY_MS"
done

echo ""
echo "========== focus-probe 结果 =========="
echo "总次数  : $N"
echo "抢焦点数: $stolen"
rate=$(awk "BEGIN { printf \"%.1f\", $stolen * 100 / $N }")
echo "抢焦点率: ${rate}%"
echo "baseline: $baseline"
echo "======================================"

if [[ "$stolen" -gt 0 ]]; then
  exit 1
else
  exit 0
fi
