#!/bin/bash
# Restart the agentdocs server on 0.0.0.0:9090 (LAN accessible).
# Run after `wsl --shutdown` / WSL restart, or to bounce the service.
set -e
cd "$(dirname "$0")"
pkill -x agentdocs 2>/dev/null || true
sleep 1
nohup env AGENTDOCS_HTTP_ADDR=:9090 ./agentdocs serve > /tmp/agentdocs-9090.log 2>&1 &
sleep 2
curl -s -m 3 http://127.0.0.1:9090/healthz && echo " <- agentdocs ready on :9090 (LAN: http://172.23.33.103:9090)"