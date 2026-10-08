#!/usr/bin/env bash
# Builds the release archive for this system, the counterpart of
# package-release.ps1:
#
#   Linux:  dist/tumble-<version>-linux-<arch>[-lgpl].tar.gz
#   macOS:  dist/tumble-<version>-macos-<arch>[-lgpl].zip
#
# 1. Builds tumble in release mode (and the desktop window with --desktop).
# 2. Makes sure vendor/ is populated (runs fetch-vendor.sh).
# 3. Stages the binaries, vendor libraries, licences, README and the
#    install and uninstall scripts, and packs them.
# 4. Unpacks the result into an empty temp folder, converts real files
#    from there, and runs install.sh and uninstall.sh against a temp home,
#    so a broken archive is caught here and not by whoever opens it.
#
# Developer tooling only. The packaged app never touches the network.
#
# Usage: scripts/package-release.sh [--lgpl] [--desktop]
#   --lgpl     package libheif without x265 (HEIC read only). Use for builds
#              you share widely.
#   --desktop  also build the desktop window (needs Bun; on Linux also the
#              WebKitGTK development packages Tauri needs). On macOS it
#              becomes Tumble.app, with tumble inside it.

set -euo pipefail

lgpl=0
desktop=0
for arg in "$@"; do
    case "$arg" in
        --lgpl) lgpl=1 ;;
        --desktop) desktop=1 ;;
        -h | --help)
            sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown option: $arg (try --help)" >&2
            exit 2
            ;;
    esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
os=$(uname -s)
case "$(uname -m)" in
    x86_64) arch=x64 ;;
    arm64 | aarch64) arch=arm64 ;;
    *) arch=$(uname -m) ;;
esac
if [ "$os" = Darwin ]; then platform=macos; else platform=linux; fi
name="tumble-$version-$platform-$arch"
if [ "$lgpl" = 1 ]; then name="$name-lgpl"; fi
dist="$root/dist"
stage="$dist/$name"
target=$(rustc -vV | sed -n 's/^host: //p')

echo "== building release"
if [ "$lgpl" = 1 ]; then scripts/fetch-vendor.sh --lgpl; else scripts/fetch-vendor.sh; fi
cargo build --release --bin tumble

echo "== staging $name"
rm -rf "$stage" "$dist/$name.tar.gz" "$dist/$name.zip"
mkdir -p "$stage/licences"

# Where tumble and the vendor libraries go inside the archive.
if [ "$os" = Darwin ] && [ "$desktop" = 1 ]; then
    app="$stage/Tumble.app/Contents/MacOS"
else
    app="$stage"
fi

if [ "$desktop" = 1 ]; then
    echo "== building the desktop window"
    (
        cd apps/desktop
        bun install --frozen-lockfile
        if [ "$os" = Darwin ]; then
            bun run tauri build --bundles app --config '{"bundle":{"active":true}}'
        else
            bun run tauri build
        fi
    )
    if [ "$os" = Darwin ]; then
        cp -R apps/desktop/src-tauri/target/release/bundle/macos/Tumble.app "$stage/"
    else
        cp apps/desktop/src-tauri/target/release/tumble-desktop "$stage/"
    fi
fi

cp target/release/tumble "$app/"
find vendor -maxdepth 1 \( -name '*.so*' -o -name '*.dylib' \) -exec cp {} "$app/" \;
cp vendor/*-LICENSE.txt "$stage/licences/"
cp LICENSE "$stage/licences/Tumble-LICENSE.txt"
cp packaging/unix/install.sh packaging/unix/uninstall.sh "$stage/"
chmod +x "$stage/install.sh" "$stage/uninstall.sh"
if [ "$os" != Darwin ]; then
    cp assets/tumble.png "$stage/"
fi
sed "s/{VERSION}/$version/" packaging/README-unix.txt > "$stage/README.txt"

# The Rust crates compiled into tumble, with their licences.
crates() {
    cargo tree "$@" --prefix none --edges normal,build --target "$target" --format '{p} | {l}' \
        | sed -e 's/ (.*)//' -e 's/ (\*)$//' | grep -v '^tumble'
}
{
    echo "Third-party software in tumble and tumble-desktop"
    echo "(Rust crates, compiled in; name version | licence)"
    echo
    {
        crates -p tumble-cli
        if [ "$desktop" = 1 ]; then crates --manifest-path apps/desktop/src-tauri/Cargo.toml; fi
    } | sort -u
    echo
    echo "Libraries shipped alongside: see the *-LICENSE.txt files in this folder."
} > "$stage/licences/THIRD-PARTY.txt"

if [ "$os" = Darwin ]; then
    # Ad-hoc signatures: Apple silicon runs nothing unsigned. Inner code
    # first, then the bundle around it.
    for f in "$app"/*.dylib "$app/tumble"; do codesign --force --sign - "$f"; done
    if [ "$desktop" = 1 ]; then codesign --force --sign - "$stage/Tumble.app"; fi
    (cd "$dist" && ditto -c -k --keepParent "$name" "$name.zip")
    archive="$dist/$name.zip"
else
    tar -czf "$dist/$name.tar.gz" -C "$dist" "$name"
    archive="$dist/$name.tar.gz"
fi

echo "== smoke test from a clean folder"
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/tumble-release-test.XXXXXX")
trap 'rm -rf "$test_dir"' EXIT
if [ "$os" = Darwin ]; then
    ditto -x -k "$archive" "$test_dir"
else
    tar -xzf "$archive" -C "$test_dir"
fi
unpacked="$test_dir/$name"
if [ "$os" = Darwin ] && [ "$desktop" = 1 ]; then
    exe="$unpacked/Tumble.app/Contents/MacOS/tumble"
    codesign --verify --deep --strict "$unpacked/Tumble.app"
else
    exe="$unpacked/tumble"
fi
work="$test_dir/files"
mkdir -p "$work"

# A small PNG and a two-page PDF, made with the packaged tumble itself.
echo '<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48"><rect width="64" height="48" fill="#7eb328"/></svg>' > "$work/card.svg"
"$exe" "$work/card.svg" --to png > /dev/null
printf '%%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R 4 0 R]/Count 2>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 72 72]>>endobj\n4 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 72 72]>>endobj\ntrailer<</Root 1 0 R>>' > "$work/doc.pdf"

# check <expected exit code> <file it must write, or -> <args...>
check() {
    local expect="$1" file="$2" code=0
    shift 2
    out=$("$exe" "$@" 2>&1) || code=$?
    if [ "$code" != "$expect" ]; then
        echo "tumble $* exited $code (expected $expect):" >&2
        echo "$out" >&2
        exit 1
    fi
    if [ "$file" != - ] && [ ! -f "$work/$file" ]; then
        echo "tumble $* did not write $file" >&2
        exit 1
    fi
    echo "  ok: tumble $*"
}
check 0 - engines
check 0 card.avif "$work/card.png" --to avif
if [ "$lgpl" = 1 ]; then
    check 3 - "$work/card.png" --to heic
else
    check 0 card.heic "$work/card.png" --to heic
fi
check 0 doc-p002.jpg "$work/doc.pdf" --to jpg
engines=$("$exe" engines)
if ! echo "$engines" | grep -q 'libheif \[ok\]' || ! echo "$engines" | grep -q 'pdfium \[ok\]'; then
    echo "packaged libraries not picked up:" >&2
    echo "$engines" >&2
    exit 1
fi

echo "== install and uninstall into a temp home"
home="$test_dir/home"
mkdir -p "$home"
run_home() {
    env -i PATH=/usr/bin:/bin HOME="$home" TUMBLE_FILE_MANAGERS=dolphin,nemo,thunar,nautilus "$@"
}
run_home sh "$unpacked/install.sh" > "$test_dir/install.log" 2>&1 || {
    cat "$test_dir/install.log" >&2
    exit 1
}
installed="$home/.local/bin/tumble"
[ -x "$installed" ] || { echo "install.sh did not link $installed" >&2; exit 1; }
run_home "$installed" "$work/card.png" --to bmp > /dev/null
[ -f "$work/card.bmp" ] || { echo "the installed tumble did not convert" >&2; exit 1; }
if run_home "$installed" menu status | grep -q 'not installed'; then
    echo "install.sh did not install the menu" >&2
    exit 1
fi
if [ "$os" = Darwin ]; then
    uninstaller="$home/Library/Application Support/Tumble/uninstall.sh"
else
    uninstaller="$home/.local/share/tumble/app/uninstall.sh"
fi
run_home sh "$uninstaller" > "$test_dir/uninstall.log" 2>&1 || {
    cat "$test_dir/uninstall.log" >&2
    exit 1
}
left=$(find "$home" -iname '*tumble*' -o -name '*.desktop' -o -name '*.nemo_action')
if [ -n "$left" ] || grep -qs 'added by Tumble' "$home/.profile" "$home/.zprofile"; then
    echo "uninstall.sh left things behind:" >&2
    echo "$left" >&2
    exit 1
fi
echo "  ok: install.sh, menu, uninstall.sh"

size=$(du -k "$archive" | cut -f1)
echo "== $archive ($((size / 1024)) MB)"
