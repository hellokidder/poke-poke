#!/usr/bin/env bash
# Cursor 项目钩子入口：固定到仓库根再执行 Python，避免 cwd/PATH 异常
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
cd "$ROOT" || exit 0
if [[ -x /usr/bin/python3 ]]; then
  exec /usr/bin/python3 "$ROOT/.cursor/hooks/pokepokeCursorStop.py"
fi
exec python3 "$ROOT/.cursor/hooks/pokepokeCursorStop.py"
