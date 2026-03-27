#!/bin/bash
# Launcher script for SysWall UI
# Script de lancement pour l'interface SysWall
cd /home/seb/Dev/SysWall/crates/ui
exec npm run tauri dev 2>/dev/null
