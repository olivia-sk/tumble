#!/bin/sh
# Installs Tumble for the current user, without sudo: copies it into your
# home folder, puts `tumble` on your PATH, adds the right-click menu and, if
# it is included, the desktop window to your apps.
#
#   macOS:  ~/Applications/Tumble.app (or ~/Library/Application Support/Tumble/app)
#   Linux:  ~/.local/share/tumble/app
#   both:   ~/.local/bin/tumble
#
# Run it from the unpacked folder:  sh install.sh
# Undo it with uninstall.sh (the last line of the output says where it is).

set -eu

here=$(cd "$(dirname "$0")" && pwd)
os=$(uname -s)
bin="$HOME/.local/bin"
marker="# added by Tumble"

if [ "$os" = Darwin ]; then
    support="$HOME/Library/Application Support/Tumble"
    if [ -d "$here/Tumble.app" ]; then
        dest="$HOME/Applications/Tumble.app"
        mkdir -p "$HOME/Applications"
        rm -rf "$dest"
        cp -R "$here/Tumble.app" "$dest"
        tumble="$dest/Contents/MacOS/tumble"
    else
        dest="$support/app"
        rm -rf "$dest"
        mkdir -p "$dest"
        cp "$here"/tumble "$here"/*.dylib "$dest/"
        tumble="$dest/tumble"
    fi
    # Files from a downloaded zip are quarantined, and macOS would refuse to
    # run the unsigned tumble the first time.
    xattr -dr com.apple.quarantine "$dest" 2> /dev/null || true
    mkdir -p "$support"
    cp "$here/uninstall.sh" "$support/uninstall.sh"
    uninstaller="$support/uninstall.sh"
    profile="$HOME/.zprofile"
else
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    dest="$data/tumble/app"
    rm -rf "$dest"
    mkdir -p "$dest"
    for f in "$here"/tumble "$here"/tumble-desktop "$here"/tumble.png "$here"/*.so* "$here"/uninstall.sh "$here"/README.txt; do
        if [ -e "$f" ]; then cp "$f" "$dest/"; fi
    done
    cp -R "$here/licences" "$dest/"
    tumble="$dest/tumble"
    uninstaller="$dest/uninstall.sh"
    profile="$HOME/.profile"
    if [ -x "$dest/tumble-desktop" ]; then
        mkdir -p "$data/applications"
        cat > "$data/applications/tumble.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Tumble
Comment=Convert images, video, audio and documents
Exec="$dest/tumble-desktop" %F
Icon=$dest/tumble.png
Terminal=false
Categories=Utility;Graphics;AudioVideo;
EOF
        if command -v update-desktop-database > /dev/null; then
            update-desktop-database "$data/applications" > /dev/null 2>&1 || true
        fi
    fi
fi

mkdir -p "$bin"
ln -sf "$tumble" "$bin/tumble"

# ~/.local/bin on PATH for new terminals.
case ":${PATH:-}:" in
    *":$bin:"*) ;;
    *)
        if ! grep -qs "$marker" "$profile"; then
            printf '\nexport PATH="$HOME/.local/bin:$PATH" %s\n' "$marker" >> "$profile"
            echo "Added ~/.local/bin to your PATH in $profile (open a new terminal to use it)."
        fi
        ;;
esac

"$tumble" menu install || true
echo

engines=$("$tumble" engines)
if ! echo "$engines" | grep -q 'ffmpeg \[ok\]'; then
    if [ "$os" = Darwin ]; then hint="brew install ffmpeg"; else hint="install ffmpeg with your package manager"; fi
    echo "For video and audio, install FFmpeg ($hint), then run: tumble menu install"
fi
if ! echo "$engines" | grep -q 'libreoffice \[ok\]'; then
    if [ "$os" = Darwin ]; then
        hint="brew install --cask libreoffice"
    else
        hint="LibreOffice 25.8 or newer from your package manager or libreoffice.org"
    fi
    echo "For documents, install LibreOffice ($hint), then run: tumble menu install"
fi

echo "Tumble is installed. To remove it, run: sh \"$uninstaller\""
