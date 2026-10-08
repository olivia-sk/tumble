#!/usr/bin/env bash
# Puts the pinned PDFium and libheif libraries into vendor/ on macOS and
# Linux; the counterpart of fetch-vendor.ps1. Every download is checked
# against its SHA-256.
#
# Developer tooling only. Tumble itself never downloads anything.
#
#   libpdfium.so / .dylib   prebuilt, from bblanchon/pdfium-binaries   BSD-3
#   libheif 1.23.6          built here from source with CMake          LGPL-3.0
#   libde265 1.1.3          HEVC decoder, built here                   LGPL-3.0
#   libx265 3.5             HEVC encoder, built here (not with --lgpl) GPL-2.0
#
# libheif is built here instead of downloaded because the conda-forge macOS
# builds link a Homebrew library by absolute path. The libraries find each
# other through an $ORIGIN / @loader_path rpath, so they only need to sit
# side by side; on Linux they need only glibc and libstdc++, on macOS only
# the system's libc++.
#
# Needs curl, tar, a C++ compiler and CMake 3.16 or newer (on macOS:
# xcode-select --install; brew install cmake).
#
# Usage: scripts/fetch-vendor.sh [--lgpl] [--force]
#   --lgpl   leave out x265: HEIC can be read but not written. Use this for
#            builds you share (see PRD section 18).
#   --force  build and download again even when up to date.

set -euo pipefail

lgpl=0
force=0
for arg in "$@"; do
    case "$arg" in
        --lgpl) lgpl=1 ;;
        --force) force=1 ;;
        -h | --help)
            sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown option: $arg (try --help)" >&2
            exit 2
            ;;
    esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
vendor="$root/vendor"
mkdir -p "$vendor"

os=$(uname -s)
arch=$(uname -m)
case "$os-$arch" in
    Linux-x86_64)
        pdfium=linux-x64
        pdfium_sha=588577cf52dabc1a444988bac841920df54cc2f141801424de97ab04f4fbb935
        ;;
    Linux-aarch64)
        pdfium=linux-arm64
        pdfium_sha=e7e2fe4686925618330103cb167950aca5a84bb00fd977a41b86be59dd1480a2
        ;;
    Darwin-arm64)
        pdfium=mac-arm64
        pdfium_sha=e98679e052c07edbb5a627980902abb823d4b3f35744d877bd21668bd9fc13ab
        ;;
    Darwin-x86_64)
        pdfium=mac-x64
        pdfium_sha=933a85a138f6027243c56bff8676375c33ceeb767401389415ffc44d689ca85d
        ;;
    *)
        echo "unsupported system: $os $arch" >&2
        exit 1
        ;;
esac

# Pinned sources. Update the URL and hash together.
pdfium_url="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8086/pdfium-$pdfium.tgz"
heif_url="https://github.com/strukturag/libheif/releases/download/v1.23.6/libheif-1.23.6.tar.gz"
heif_sha=4484346dc5995319dbc11e3a1c35d0a2ec46511ce370900869337fd2c7033125
de265_url="https://github.com/strukturag/libde265/releases/download/v1.1.3/libde265-1.1.3.tar.gz"
de265_sha=554228bd17788c99a7e63b37ab5634722190e6e2bf60c1dcb01cef328e133905
x265_url="https://download.videolan.org/pub/videolan/x265/x265_3.5.tar.gz"
x265_sha=e70a3335cacacbba0b3a20ec6fecd6783932288ebc8163ad74bcc9606477cae8

if [ "$os" = Darwin ]; then
    lib() { echo "lib$1.$2.dylib"; } # lib heif 1 -> libheif.1.dylib
    pdfium_lib=libpdfium.dylib
    jobs=$(sysctl -n hw.ncpu)
else
    lib() { echo "lib$1.so.$2"; } # lib heif 1 -> libheif.so.1
    pdfium_lib=libpdfium.so
    jobs=$(nproc)
fi

sha256() {
    if command -v sha256sum > /dev/null; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

work=$(mktemp -d "${TMPDIR:-/tmp}/tumble-vendor.XXXXXX")
trap 'rm -rf "$work"' EXIT

# download <url> <sha256>: into $work, checked; prints the file's path.
download() {
    local file="$work/$(basename "$1")"
    curl -fsSL --retry 3 -o "$file" "$1"
    local actual
    actual=$(sha256 "$file")
    if [ "$actual" != "$2" ]; then
        echo "$(basename "$1"): SHA-256 mismatch. Expected $2, got $actual" >&2
        exit 1
    fi
    echo "$file"
}

# up_to_date <stamp> <key> <files...>: whether a previous run made these.
up_to_date() {
    local stamp="$vendor/$1.sha256" key="$2"
    shift 2
    [ "$force" = 0 ] && [ -f "$stamp" ] && [ "$(cat "$stamp")" = "$key" ] || return 1
    for f in "$@"; do [ -f "$vendor/$f" ] || return 1; done
}

# --- PDFium ------------------------------------------------------------------

if up_to_date pdfium "$pdfium_sha" "$pdfium_lib"; then
    echo "pdfium ($pdfium): up to date"
else
    echo "pdfium ($pdfium): downloading"
    archive=$(download "$pdfium_url" "$pdfium_sha")
    mkdir -p "$work/pdfium"
    tar -xzf "$archive" -C "$work/pdfium"
    cp -f "$work/pdfium/lib/$pdfium_lib" "$vendor/$pdfium_lib"
    cp -f "$work/pdfium/LICENSE" "$vendor/pdfium-LICENSE.txt"
    printf '%s' "$pdfium_sha" > "$vendor/pdfium.sha256"
    echo "pdfium ($pdfium): ok"
fi

# --- libheif -------------------------------------------------------------------

heif_files=("$(lib heif 1)" "$(lib de265 0)")
key="$heif_sha-$de265_sha"
if [ "$lgpl" = 0 ]; then
    heif_files+=("$(lib x265 199)")
    key="$key-$x265_sha"
fi

# Switching to --lgpl must not leave the GPL encoder behind.
if [ "$lgpl" = 1 ]; then
    rm -f "$vendor/$(lib x265 199)" "$vendor/x265-LICENSE.txt"
fi

if up_to_date libheif "$key" "${heif_files[@]}"; then
    echo "libheif: up to date"
else
    command -v cmake > /dev/null || {
        echo "CMake is needed to build libheif (macOS: brew install cmake; Linux: your package manager)" >&2
        exit 1
    }
    prefix="$work/prefix"
    common=(
        -DCMAKE_BUILD_TYPE=Release
        -DCMAKE_INSTALL_PREFIX="$prefix"
        -DCMAKE_INSTALL_LIBDIR=lib
        -DCMAKE_PREFIX_PATH="$prefix"
        -DCMAKE_POLICY_VERSION_MINIMUM=3.5
        -DBUILD_SHARED_LIBS=ON
    )
    if [ "$os" = Darwin ]; then
        common+=(
            -DCMAKE_INSTALL_NAME_DIR=@rpath
            -DCMAKE_INSTALL_RPATH=@loader_path
            -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0
            -DCMAKE_OSX_ARCHITECTURES="$arch"
        )
    else
        common+=(-DCMAKE_INSTALL_RPATH='$ORIGIN')
    fi
    export PKG_CONFIG_PATH="$prefix/lib/pkgconfig"

    # build <name> <source dir> <cmake options...>
    build() {
        local name="$1" src="$2"
        shift 2
        echo "$name: building"
        cmake -S "$src" -B "$work/build-$name" "${common[@]}" "$@" > "$work/$name.log" 2>&1 \
            && cmake --build "$work/build-$name" --parallel "$jobs" >> "$work/$name.log" 2>&1 \
            && cmake --install "$work/build-$name" >> "$work/$name.log" 2>&1 \
            || {
                tail -40 "$work/$name.log" >&2
                echo "$name: build failed" >&2
                exit 1
            }
    }

    tar -xzf "$(download "$de265_url" "$de265_sha")" -C "$work"
    build libde265 "$work/libde265-1.1.3" -DENABLE_SDL=OFF -DENABLE_DECODER=OFF -DENABLE_ENCODER=OFF

    with_x265=OFF
    if [ "$lgpl" = 0 ]; then
        tar -xzf "$(download "$x265_url" "$x265_sha")" -C "$work"
        # No assembly: it needs nasm on x86 and does not build on Apple
        # silicon; encoding is slower but still fine for photos.
        build x265 "$work/x265_3.5/source" -DENABLE_SHARED=ON -DENABLE_CLI=OFF -DENABLE_ASSEMBLY=OFF
        with_x265=ON
    fi

    tar -xzf "$(download "$heif_url" "$heif_sha")" -C "$work"
    # Only libde265 and x265: everything else libheif can use is off, so it
    # never picks up whatever else happens to be installed on this machine.
    build libheif "$work/libheif-1.23.6" \
        -DWITH_LIBDE265=ON -DWITH_LIBDE265_PLUGIN=OFF \
        -DWITH_X265="$with_x265" -DWITH_X265_PLUGIN=OFF \
        -DENABLE_PLUGIN_LOADING=OFF \
        -DWITH_AOM_DECODER=OFF -DWITH_AOM_ENCODER=OFF -DWITH_DAV1D=OFF -DWITH_RAV1E=OFF \
        -DWITH_SvtEnc=OFF -DWITH_X264=OFF -DWITH_OpenH264_DECODER=OFF -DWITH_KVAZAAR=OFF \
        -DWITH_UVG266=OFF -DWITH_VVDEC=OFF -DWITH_VVENC=OFF -DWITH_FFMPEG_DECODER=OFF \
        -DWITH_JPEG_DECODER=OFF -DWITH_JPEG_ENCODER=OFF \
        -DWITH_OpenJPEG_DECODER=OFF -DWITH_OpenJPEG_ENCODER=OFF -DWITH_OPENJPH_ENCODER=OFF \
        -DWITH_LIBSHARPYUV=OFF -DWITH_UNCOMPRESSED_CODEC=OFF -DWITH_HEADER_COMPRESSION=OFF \
        -DWITH_EXAMPLES=OFF -DWITH_GDK_PIXBUF=OFF -DBUILD_TESTING=OFF -DBUILD_DOCUMENTATION=OFF

    for f in "${heif_files[@]}"; do
        cp -fL "$prefix/lib/$f" "$vendor/$f"
    done

    # Every dependency must be one of ours or part of the system.
    for f in "${heif_files[@]}"; do
        if [ "$os" = Darwin ]; then
            deps=$(otool -L "$vendor/$f" | tail -n +2 | awk '{print $1}')
            bad=$(echo "$deps" | grep -vE '^(@rpath/lib(heif|de265|x265)\.|/usr/lib/|/System/)' || true)
        else
            deps=$(readelf -d "$vendor/$f" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
            bad=$(echo "$deps" | grep -vE '^(lib(heif|de265|x265)\.so|lib(c|m|dl|pthread|rt|gcc_s|stdc\+\+)\.so|ld-linux)' || true)
        fi
        if [ -n "$bad" ]; then
            echo "$f links libraries that are not shipped: $bad" >&2
            exit 1
        fi
    done

    cp -f "$work/libheif-1.23.6/COPYING" "$vendor/libheif-LICENSE.txt"
    cp -f "$work/libde265-1.1.3/COPYING" "$vendor/libde265-LICENSE.txt"
    if [ "$lgpl" = 0 ]; then
        cp -f "$work/x265_3.5/COPYING" "$vendor/x265-LICENSE.txt"
    fi
    printf '%s' "$key" > "$vendor/libheif.sha256"
    echo "libheif: ok"
fi

# macOS refuses unsigned code on Apple silicon; an ad-hoc signature is enough.
if [ "$os" = Darwin ]; then
    for f in "$vendor"/*.dylib; do codesign --force --sign - "$f" 2> /dev/null; done
fi

count=$(find "$vendor" -maxdepth 1 \( -name '*.so*' -o -name '*.dylib' \) | wc -l | tr -d ' ')
echo "vendor/: $count libraries"
