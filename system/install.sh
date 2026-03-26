#!/bin/bash
set -e

echo "=== Installation de SysWall ==="

# Build
echo "[1/6] Compilation du daemon..."
cargo build --release -p syswall-daemon

# Copie du binaire
echo "[2/6] Installation du binaire..."
sudo cp target/release/syswall-daemon /usr/bin/
sudo chmod 755 /usr/bin/syswall-daemon

# Creation des repertoires
echo "[3/6] Creation des repertoires..."
sudo mkdir -p /etc/syswall /var/lib/syswall /var/log/syswall /var/run/syswall

# Configuration
echo "[4/6] Installation de la configuration..."
if [ ! -f /etc/syswall/config.toml ]; then
    sudo cp config/default.toml /etc/syswall/config.toml
    echo "  -> config/default.toml copie vers /etc/syswall/config.toml"
else
    echo "  -> /etc/syswall/config.toml existe deja, conserve"
fi

# Groupe syswall pour le socket Unix
echo "[5/6] Creation du groupe syswall..."
if ! getent group syswall > /dev/null 2>&1; then
    sudo groupadd syswall
    echo "  -> Groupe 'syswall' cree"
else
    echo "  -> Groupe 'syswall' existe deja"
fi
sudo usermod -aG syswall "$USER"
echo "  -> Utilisateur '$USER' ajoute au groupe 'syswall'"

# Service systemd
echo "[6/6] Installation du service systemd..."
sudo cp system/syswall.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable syswall

echo ""
echo "=== Installation terminee ==="
echo ""
echo "Commandes utiles :"
echo "  sudo systemctl start syswall    # Demarrer le daemon"
echo "  sudo systemctl status syswall   # Verifier le statut"
echo "  sudo journalctl -u syswall -f   # Voir les logs"
echo ""
echo "NOTE: Deconnectez-vous puis reconnectez-vous pour"
echo "      que l'appartenance au groupe 'syswall' prenne effet."
