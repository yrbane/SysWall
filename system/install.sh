#!/bin/bash
set -e

echo "=== Installation de SysWall ==="

# Racine du depot resolue une seule fois, en absolu : on se place dedans pour que
# tous les chemins relatifs (target/, crates/ui, config/, system/) restent valides
# quel que soit le repertoire d'invocation.
# Repo root resolved once, absolute: cd into it so every relative path stays valid
# regardless of the caller's working directory.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT"

# Detection de la distribution : adapte l'installation au systeme courant.
# Distribution detection: adapt the install to the current system.
DISTRO_ID="unknown"
DISTRO_LIKE=""
DISTRO_PRETTY=""
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    DISTRO_ID="${ID:-unknown}"
    DISTRO_LIKE="${ID_LIKE:-}"
    DISTRO_PRETTY="${PRETTY_NAME:-$DISTRO_ID}"
fi
echo "Distribution detectee : ${DISTRO_PRETTY:-$DISTRO_ID}"

# SysWall est un service systemd : sans systemctl, pas d'installation propre.
# SysWall is a systemd service: without systemctl there is no clean install.
if ! command -v systemctl > /dev/null 2>&1; then
    echo "ERREUR : systemd (systemctl) introuvable. SysWall requiert systemd." >&2
    exit 1
fi

# Sur la famille Arch, un paquet natif existe : on le signale sans l'imposer.
# On the Arch family, a native package exists: mention it without forcing it.
case " $DISTRO_ID $DISTRO_LIKE " in
    *" arch "*)
        echo "  -> Astuce Arch : paquet natif disponible (cd system/arch && makepkg -si)"
        ;;
esac

# Authentification sudo en amont : l'install ecrit dans /usr/bin, /etc et
# /etc/systemd. Autant demander le mot de passe maintenant que de bloquer apres
# plusieurs minutes de compilation. Amorce aussi le cache sudo pour la suite.
# Upfront sudo auth: the install writes to system dirs; better prompt now than
# block after minutes of compilation. Also primes the sudo timestamp cache.
echo "Authentification administrateur requise (sudo)..."
if ! sudo -v; then
    echo "ERREUR : privileges administrateur requis. Relancez dans un terminal" >&2
    echo "        interactif ou l'invite sudo peut recevoir le mot de passe." >&2
    exit 1
fi

# Build daemon
echo "[1/9] Compilation du daemon..."
cargo build --release -p syswall-daemon

# Build UI : isole dans un sous-shell (le cd n'affecte pas le script) et NON bloquant.
# Un echec de compilation de l'interface ne doit pas empecher l'installation du
# daemon et le demarrage du service ; l'UI reste lancable via `npm run tauri dev`.
# UI build: isolated in a subshell (its cd never leaks) and NON-fatal. A UI build
# failure must not block daemon install and service start; the UI can still run via dev.
# --no-bundle : on ne veut que le binaire a copier dans /usr/bin. Le bundling
# .deb/.rpm/.AppImage echoue hors ligne (linuxdeploy telecharge des outils) et
# n'est pas necessaire pour une install source. On appelle le CLI tauri en direct :
# via `npm run ... -- --no-bundle`, le `--` de npm ferait avaler le flag comme
# argument passthrough et le bundling tournerait quand meme.
# --no-bundle: we only need the binary to copy into /usr/bin. The .deb/.rpm/.AppImage
# bundling fails offline (linuxdeploy) and is not needed. We call the tauri CLI
# directly: through `npm run ... -- --no-bundle`, npm's `--` swallows the flag as a
# passthrough arg and bundling would still run.
echo "[2/9] Compilation de l'interface..."
if ( cd "$ROOT/crates/ui" && npm install && ./node_modules/.bin/tauri build --no-bundle ); then
    echo "  -> interface compilee"
else
    echo "  -> ATTENTION: build UI echoue, interface lancable via: cd crates/ui && npm run tauri dev"
fi
# Tauri produit le binaire dans le target du workspace, nomme d'apres productName (ui).
# Tauri outputs the binary in the workspace target, named after productName (ui).
UI_BIN="$ROOT/target/release/ui"
if [ ! -f "$UI_BIN" ]; then
    UI_BIN=$(find "$ROOT/target/release/" -maxdepth 1 -type f -executable \( -name "ui" -o -name "syswall-ui" \) 2>/dev/null | head -1)
fi

# Copie des binaires
echo "[3/9] Installation des binaires..."
sudo cp target/release/syswall-daemon /usr/bin/
sudo chmod 755 /usr/bin/syswall-daemon
if [ -n "$UI_BIN" ] && [ -f "$UI_BIN" ]; then
    sudo cp "$UI_BIN" /usr/bin/syswall-ui
    sudo chmod 755 /usr/bin/syswall-ui
    echo "  -> syswall-ui installe"
else
    echo "  -> ATTENTION: binaire UI non trouve, lancer avec: cd crates/ui && npm run tauri dev"
fi

# Creation des repertoires
echo "[4/9] Creation des repertoires..."
sudo mkdir -p /etc/syswall /var/lib/syswall /var/log/syswall /var/run/syswall

# Configuration
echo "[5/9] Installation de la configuration..."
if [ ! -f /etc/syswall/config.toml ]; then
    sudo cp config/default.toml /etc/syswall/config.toml
    echo "  -> config/default.toml copie vers /etc/syswall/config.toml"
else
    echo "  -> /etc/syswall/config.toml existe deja, conserve"
fi

# Groupe et utilisateur systeme syswall (le unit tourne en User=syswall)
echo "[6/9] Creation du groupe et de l'utilisateur syswall..."
if ! getent group syswall > /dev/null 2>&1; then
    sudo groupadd --system syswall
    echo "  -> Groupe 'syswall' cree"
else
    echo "  -> Groupe 'syswall' existe deja"
fi
# Utilisateur systeme dedie, sans shell ni home : exige par User=syswall dans le unit.
# Dedicated system user, no shell nor home: required by User=syswall in the unit.
if ! getent passwd syswall > /dev/null 2>&1; then
    NOLOGIN=$(command -v nologin || echo /usr/sbin/nologin)
    sudo useradd --system --no-create-home --shell "$NOLOGIN" --gid syswall syswall
    echo "  -> Utilisateur systeme 'syswall' cree"
else
    echo "  -> Utilisateur systeme 'syswall' existe deja"
fi
sudo usermod -aG syswall "$USER"
echo "  -> Utilisateur '$USER' ajoute au groupe 'syswall'"

# Service systemd
echo "[7/9] Installation du service systemd..."
sudo cp system/syswall.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable syswall

# Desktop entry pour GNOME/KDE
echo "[8/9] Installation du raccourci bureau..."
sudo cp system/syswall.desktop /usr/share/applications/
sudo update-desktop-database /usr/share/applications/ 2>/dev/null || true

# Demarrage du service : install ET demarrage en une seule commande.
# Start the service: install AND start in a single command.
echo "[9/9] Demarrage du service..."
sudo systemctl restart syswall
sudo systemctl status syswall --no-pager || true

echo ""
echo "=== Installation terminee ==="
echo ""
echo "Commandes utiles :"
echo "  sudo systemctl status syswall      # Verifier le statut"
echo "  sudo systemctl restart syswall     # Redemarrer le daemon"
echo "  sudo journalctl -u syswall -f      # Voir les logs"
echo "  syswall-ui                         # Lancer l'interface"
echo ""
echo "SysWall est disponible dans 'Afficher les applications' (GNOME)"
echo ""
echo "NOTE: Deconnectez-vous puis reconnectez-vous pour que l'appartenance"
echo "      au groupe 'syswall' prenne effet (acces au socket depuis l'UI)."
