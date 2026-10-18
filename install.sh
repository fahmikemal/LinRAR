#!/usr/bin/env bash
# Install LinRAR to current user's desktop environment

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.local/share/applications"

# Symlink executable
ln -sf "$SCRIPT_DIR/linrar" "$HOME/.local/bin/linrar"

# Install icons in all standard resolutions for GNOME / Ubuntu Dock
SIZES=(16 24 32 48 64 128 256)
for SZ in "${SIZES[@]}"; do
    TARGET_DIR="$HOME/.local/share/icons/hicolor/${SZ}x${SZ}/apps"
    mkdir -p "$TARGET_DIR"
    SRC_ICON="$SCRIPT_DIR/linrar_app/assets/icons/hicolor/${SZ}x${SZ}/apps/linrar.png"
    if [ -f "$SRC_ICON" ]; then
        cp "$SRC_ICON" "$TARGET_DIR/linrar.png"
    else
        cp "$SCRIPT_DIR/linrar_app/assets/icons/linrar.png" "$TARGET_DIR/linrar.png"
    fi
done

# Also put in pixmaps for legacy fallbacks
mkdir -p "$HOME/.local/share/pixmaps"
cp "$SCRIPT_DIR/linrar_app/assets/icons/linrar.png" "$HOME/.local/share/pixmaps/linrar.png"

# Install .desktop entry with StartupWMClass to guarantee dock matching
cat > "$HOME/.local/share/applications/linrar.desktop" <<EOF
[Desktop Entry]
Name=LinRAR
Comment=Universal Linux Archive Manager (v0.1.0)
GenericName=Archive Manager
Exec=$HOME/.local/bin/linrar %F
Icon=linrar
Terminal=false
Type=Application
Categories=Utility;Archiving;Compression;GTK;
MimeType=application/x-rar;application/x-rar-compressed;application/zip;application/x-tar;application/x-7z-compressed;application/gzip;application/x-bzip2;application/x-xz;
StartupNotify=true
StartupWMClass=linrar
EOF

# Update desktop and icon databases
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
fi

echo "LinRAR successfully installed to ~/.local/bin/linrar and application menu!"
echo "Dock icons and desktop integration refreshed."
