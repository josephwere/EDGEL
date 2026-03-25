#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo test
node --check frontend/app.js
node --check frontend/activity-bar.js
node --check frontend/sidebar.js
node --check frontend/editor.js
node --check frontend/terminal.js
node --check frontend/debug-panel.js
node --check frontend/config.js
bash -n edgel.sh
bash -n scripts/install.sh
bash -n scripts/release.sh
bash -n scripts/dev.sh
