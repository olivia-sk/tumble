#!/bin/sh
# Removes everything install.sh and Tumble created: the program, the
# right-click menu, the desktop window's launcher and data, settings and
# presets, logs, the `tumble` link and the PATH line. FFmpeg and LibreOffice
# are separate programs and are left alone.

set -u

os=$(uname -s)
bin="$HOME/.local/bin"
marker="# added by Tumble"

if [ "$os" = Darwin ]; then
    support="$HOME/Library/Application Support/Tumble"
    if [ -x "$HOME/Applications/Tumble.app/Contents/MacOS/tumble" ]; then
        tumble="$HOME/Applications/Tumble.app/Contents/MacOS/tumble"
    else
        tumble="$support/app/tumble"
    fi
    profile="$HOME/.zprofile"
else
    data="${XDG_DATA_HOME:-$HOME/.local/share}"
    config="${XDG_CONFIG_HOME:-$HOME/.config}"
    tumble="$data/tumble/app/tumble"
    profile="$HOME/.profile"
fi

if [ -x "$tumble" ]; then
    "$tumble" menu uninstall
fi

# The link, only if it is ours.
link=$(readlink "$bin/tumble" 2> /dev/null || true)
if [ "$link" = "$tumble" ]; then
    rm -f "$bin/tumble"
fi

# The PATH line install.sh added.
if grep -qs "$marker" "$profile"; then
    grep -v "$marker" "$profile" > "$profile.tumble-tmp" && cat "$profile.tumble-tmp" > "$profile"
    rm -f "$profile.tumble-tmp"
fi

if [ "$os" = Darwin ]; then
    rm -rf "$HOME/Applications/Tumble.app" \
        "$support" \
        "$HOME/Library/Logs/Tumble" \
        "$HOME/Library/Application Support/dev.tumble.desktop" \
        "$HOME/Library/Caches/dev.tumble.desktop" \
        "$HOME/Library/WebKit/dev.tumble.desktop"
else
    rm -f "$data/applications/tumble.desktop"
    rm -rf "$data/tumble" \
        "$config/tumble" \
        "$data/dev.tumble.desktop" \
        "${XDG_CACHE_HOME:-$HOME/.cache}/dev.tumble.desktop"
fi

echo "Tumble is removed."
