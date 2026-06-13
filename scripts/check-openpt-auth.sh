#!/usr/bin/env bash
set -euo pipefail

host="${1:-192.168.1.174}"

ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" '
  echo "OpenPT paths:"
  find /home/skylarenns -maxdepth 4 -iname "*openpt*" 2>/dev/null | head -20
  echo
  echo "Redacted auth env keys:"
  if [ -f /home/skylarenns/.config/gmbl-auth.env ]; then
    sed -E "s/(^[A-Za-z_][A-Za-z0-9_]*=).*/\1REDACTED/" /home/skylarenns/.config/gmbl-auth.env
  fi
'
