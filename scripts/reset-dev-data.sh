#!/usr/bin/env bash
set -euo pipefail
HOME_DIR="${KLYNTBOT_HOME:-$HOME/.klyntbot-dev}"
echo "Wiping ${HOME_DIR}/data.db + lance/"
rm -f "${HOME_DIR}/data.db" "${HOME_DIR}/data.db-wal" "${HOME_DIR}/data.db-shm"
rm -rf "${HOME_DIR}/lance/"
echo "Done. Config + sessions/ + KLYNTBOT.md + AGENTS.md preserved."
