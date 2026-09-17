#!/usr/bin/env bash
# Instalador de halo: descarga el código fuente, lo compila y coloca el
# binario en el PATH del usuario. Pensado para ejecutarse vía:
#   curl -fsSL https://raw.githubusercontent.com/millerbermeo/accesskit/main/install.sh | bash
set -euo pipefail

REPO_URL="https://github.com/millerbermeo/accesskit.git"
SRC_DIR="${HALO_SRC_DIR:-$HOME/.local/share/halo/src}"
BIN_DIR="${HALO_BIN_DIR:-$HOME/.local/bin}"
ICON_DIR="$HOME/.local/share/icons"
DESKTOP_DIR="$HOME/.local/share/applications"

echo "==> Instalando halo"

if ! command -v git >/dev/null 2>&1; then
    echo "error: falta 'git'. Instálalo con el gestor de paquetes de tu sistema." >&2
    exit 1
fi

# El icono de bandeja (GTK + libappindicator) necesita estas libs de desarrollo.
if command -v apt-get >/dev/null 2>&1; then
    if ! pkg-config --exists gtk+-3.0 xdo 2>/dev/null; then
        echo "==> Instalando dependencias de sistema (gtk3, libxdo, appindicator)..."
        sudo apt-get install -y libgtk-3-dev libxdo-dev libayatana-appindicator3-dev \
            || sudo apt-get install -y libgtk-3-dev libxdo-dev libappindicator3-dev
    fi
else
    echo "==> Aviso: asegúrate de tener instaladas las libs de desarrollo de gtk3, libxdo y libappindicator/ayatana-appindicator para tu distro."
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "==> Rust/cargo no encontrado, instalando con rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

if [ -d "$SRC_DIR/.git" ]; then
    echo "==> Actualizando código fuente en $SRC_DIR"
    git -C "$SRC_DIR" fetch --depth 1 origin main
    git -C "$SRC_DIR" reset --hard origin/main
else
    echo "==> Descargando código fuente en $SRC_DIR"
    mkdir -p "$(dirname "$SRC_DIR")"
    git clone --depth 1 "$REPO_URL" "$SRC_DIR"
fi

echo "==> Compilando (release, puede tardar unos minutos)"
cargo build --release --manifest-path "$SRC_DIR/Cargo.toml"

mkdir -p "$BIN_DIR"
install -m 755 "$SRC_DIR/target/release/halo" "$BIN_DIR/halo"

echo "==> Instalando icono y lanzador de escritorio"
mkdir -p "$ICON_DIR" "$DESKTOP_DIR"
install -m 644 "$SRC_DIR/logo.png" "$ICON_DIR/halo.png"
cat > "$DESKTOP_DIR/halo.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=halo
Comment=Widgets circulares para monitorizar CPU, RAM, disco y GPU
Exec=$BIN_DIR/halo
Icon=$ICON_DIR/halo.png
Terminal=false
Categories=Utility;System;Monitor;
EOF
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$ICON_DIR" 2>/dev/null || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$DESKTOP_DIR" 2>/dev/null || true

echo "==> halo instalado en $BIN_DIR/halo"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        echo "==> '$BIN_DIR' no está en tu PATH. Agrega esto a tu ~/.bashrc o ~/.zshrc:"
        echo "    export PATH=\"$BIN_DIR:\$PATH\""
        ;;
esac

echo "==> Listo. Ejecuta 'halo' para iniciar."
