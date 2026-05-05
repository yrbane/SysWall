#!/bin/sh
# Verifie que syswall.service contient toutes les directives de durcissement attendues.
# Verifies that syswall.service contains all expected hardening directives.
set -eu

UNIT_FILE="${1:-system/syswall.service}"

if [ ! -f "$UNIT_FILE" ]; then
    echo "ERROR: $UNIT_FILE not found" >&2
    exit 1
fi

EXPECTED="User=syswall
Group=syswall
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictNamespaces=true
LockPersonality=true
RestrictRealtime=true
RestrictAddressFamilies=AF_UNIX AF_NETLINK AF_INET AF_INET6
SystemCallFilter=@system-service @network-io @file-system ~@privileged ~@resources ~@obsolete
SystemCallArchitectures=native
AmbientCapabilities=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN
CapabilityBoundingSet=CAP_NET_ADMIN CAP_NET_RAW CAP_BPF CAP_PERFMON CAP_SYS_PTRACE CAP_DAC_READ_SEARCH CAP_CHOWN
ConfigurationDirectory=syswall
LogsDirectory=syswall
StateDirectory=syswall
RuntimeDirectory=syswall"

missing=0
echo "$EXPECTED" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    if ! grep -Fxq "$line" "$UNIT_FILE"; then
        echo "MISSING: $line" >&2
    fi
done

# Recount in main shell to get exit status
count=$(echo "$EXPECTED" | while IFS= read -r line; do
    [ -z "$line" ] && continue
    grep -Fxq "$line" "$UNIT_FILE" || echo X
done | wc -l)

if [ "$count" -gt 0 ]; then
    echo "FAIL: $count directive(s) missing from $UNIT_FILE" >&2
    exit 1
fi

echo "OK: all hardening directives present in $UNIT_FILE"
