#!/bin/bash
set -e

echo "=== Installation de SysWall ==="

# Build daemon
echo "[1/8] Compilation du daemon..."
cargo build --release -p syswall-daemon

# Build UI
echo "[2/8] Compilation de l'interface..."
cd crates/ui && npm install && npm run tauri build 2>/dev/null && cd ../..
UI_BIN="crates/ui/src-tauri/target/release/syswall-ui"
if [ ! -f "$UI_BIN" ]; then
    # Tauri might name it differently
    UI_BIN=$(find crates/ui/src-tauri/target/release/ -maxdepth 1 -type f -executable -name "ui" -o -name "syswall*" 2>/dev/null | head -1)
fi

# Copie des binaires
echo "[3/8] Installation des binaires..."
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
echo "[4/8] Creation des repertoires..."
sudo mkdir -p /etc/syswall /var/lib/syswall /var/log/syswall /var/run/syswall

# Configuration
echo "[5/8] Installation de la configuration..."
if [ ! -f /etc/syswall/config.toml ]; then
    sudo cp config/default.toml /etc/syswall/config.toml
    echo "  -> config/default.toml copie vers /etc/syswall/config.toml"
else
    echo "  -> /etc/syswall/config.toml existe deja, conserve"
fi

# Groupe syswall pour le socket Unix
echo "[6/8] Creation du groupe syswall..."
if ! getent group syswall > /dev/null 2>&1; then
    sudo groupadd syswall
    echo "  -> Groupe 'syswall' cree"
else
    echo "  -> Groupe 'syswall' existe deja"
fi
sudo usermod -aG syswall "$USER"
echo "  -> Utilisateur '$USER' ajoute au groupe 'syswall'"

# Service systemd
echo "[7/8] Installation du service systemd..."
sudo cp system/syswall.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable syswall

# Desktop entry pour GNOME/KDE
echo "[8/8] Installation du raccourci bureau..."
sudo cp system/syswall.desktop /usr/share/applications/
sudo update-desktop-database /usr/share/applications/ 2>/dev/null || true

echo ""
echo "=== Installation terminee ==="
echo ""
echo "Commandes utiles :"
echo "  sudo systemctl start syswall       # Demarrer le daemon"
echo "  sudo systemctl status syswall      # Verifier le statut"
echo "  sudo journalctl -u syswall -f      # Voir les logs"
echo "  syswall-ui                         # Lancer l'interface"
echo ""
echo "SysWall est disponible dans 'Afficher les applications' (GNOME)"
echo ""
echo "NOTE: Deconnectez-vous puis reconnectez-vous pour"
echo "      que l'appartenance au groupe 'syswall' prenne effet."
