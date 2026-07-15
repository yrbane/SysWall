#!/bin/sh
# Verifie que le service et les installeurs sont coherents avec le unit durci :
#   - le unit pointe vers la config via SYSWALL_CONFIG (sinon le daemon cherche
#     config/default.toml relatif au cwd / et crashe au demarrage systemd) ;
#   - les installeurs creent l'utilisateur systeme 'syswall' exige par User=syswall
#     (le groupe seul ne suffit pas : systemctl start echoue sans l'utilisateur).
#
# Verifies the service unit and installers stay consistent with the hardened unit:
#   - the unit references the config through SYSWALL_CONFIG (otherwise the daemon
#     looks for config/default.toml relative to cwd / and crashes under systemd);
#   - installers create the 'syswall' system user required by User=syswall
#     (the group alone is not enough: systemctl start fails without the user).
set -eu

ROOT="${1:-.}"
UNIT_FILE="$ROOT/system/syswall.service"
INSTALL_SH="$ROOT/system/install.sh"
ARCH_INSTALL="$ROOT/system/arch/syswall.install"

fail=0

check_contains() {
    # $1: fichier / file, $2: motif fixe / fixed pattern, $3: message
    if [ ! -f "$1" ]; then
        echo "ERROR: $1 not found" >&2
        fail=$((fail + 1))
        return
    fi
    if ! grep -Fq "$2" "$1"; then
        echo "MISSING in $1: $3" >&2
        fail=$((fail + 1))
    fi
}

check_matches() {
    # Comme check_contains mais motif etendu ancre : distingue une vraie
    # commande d'une simple ligne d'aide echo (indentee).
    # Like check_contains but anchored ERE: tells a real command apart from an
    # indented echo help line.
    # $1: fichier / file, $2: motif ERE / ERE pattern, $3: message
    if [ ! -f "$1" ]; then
        echo "ERROR: $1 not found" >&2
        fail=$((fail + 1))
        return
    fi
    if ! grep -Eq "$2" "$1"; then
        echo "MISSING in $1: $3" >&2
        fail=$((fail + 1))
    fi
}

# Le unit doit injecter le chemin de config absolu.
# The unit must inject the absolute config path.
check_contains "$UNIT_FILE" "Environment=SYSWALL_CONFIG=/etc/syswall/config.toml" \
    "Environment=SYSWALL_CONFIG=/etc/syswall/config.toml"

# Les installeurs doivent creer l'utilisateur systeme 'syswall'.
# Installers must create the 'syswall' system user.
check_contains "$INSTALL_SH" "useradd" "creation de l'utilisateur systeme syswall (useradd)"
check_contains "$ARCH_INSTALL" "useradd" "creation de l'utilisateur systeme syswall (useradd)"

# install.sh doit detecter la distribution pour adapter l'installation.
# install.sh must detect the distribution to adapt the install.
check_contains "$INSTALL_SH" "/etc/os-release" \
    "detection de la distribution via /etc/os-release"

# install.sh doit demarrer le service lui-meme (une seule commande installe ET
# demarre). Motif ancre : la ligne d'aide echo indentee ne doit pas compter.
# install.sh must start the service itself (one command installs AND starts).
# Anchored pattern: the indented echo help line must not count.
check_matches "$INSTALL_SH" "^[[:space:]]*sudo systemctl (start|restart) syswall" \
    "demarrage du service par le script (sudo systemctl start|restart syswall)"

if [ "$fail" -gt 0 ]; then
    echo "FAIL: $fail probleme(s) de coherence service/installeurs" >&2
    exit 1
fi

echo "OK: service unit et installeurs coherents (SYSWALL_CONFIG + utilisateur syswall)"
