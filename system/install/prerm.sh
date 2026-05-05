#!/bin/sh
# A l'uninstall, on conserve l'utilisateur (convention Linux).
# On uninstall, the user is preserved (Linux convention).
set -eu
if command -v systemctl >/dev/null; then
    systemctl stop syswall.service || true
    systemctl disable syswall.service || true
fi
