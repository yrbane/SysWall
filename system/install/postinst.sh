#!/bin/sh
# Cree l'utilisateur et le groupe systeme syswall si absents.
# Create the syswall system user and group if missing.
set -eu

if ! getent group syswall >/dev/null; then
    groupadd --system syswall
fi

if ! getent passwd syswall >/dev/null; then
    useradd --system --gid syswall \
        --home-dir /var/lib/syswall \
        --shell /usr/sbin/nologin syswall
fi

# Cree les repertoires si absents et reattribue la propriete.
# Create directories if missing and reassign ownership.
for d in /var/lib/syswall /var/log/syswall /etc/syswall; do
    if [ ! -d "$d" ]; then
        install -d -m 0750 -o syswall -g syswall "$d"
    else
        chown -R syswall:syswall "$d"
        chmod 0750 "$d"
    fi
done

# Recharge systemd au cas ou le service unit a change.
# Reload systemd in case the service unit changed.
if command -v systemctl >/dev/null; then
    systemctl daemon-reload || true
fi
