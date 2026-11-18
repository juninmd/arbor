#!/usr/bin/env bash
# Empacota o binário de release desta plataforma em dist/:
#   arbor-<v>-<so>-<arq>[.exe]   binário puro, o que a atualização automática baixa
#   Linux:   arbor-<v>-linux-<arq>.tar.gz   (binário, .desktop e ícone)
#   macOS:   arbor-<v>-macos-<arq>.zip      (arbor.app)
#   Windows: arbor-<v>-windows-<arq>.zip    (arbor.exe portátil)
set -euo pipefail
V="${1:?versão}"
TARGET="${2:-}"
ARCH="${3:-$(uname -m)}"
case "$ARCH" in arm64|aarch64) ARCH=aarch64 ;; x86_64|amd64|AMD64) ARCH=x86_64 ;; esac
BIN_DIR="target/${TARGET:+$TARGET/}release"
mkdir -p dist
case "$(uname -s)" in
  Linux)
    OS=linux
    cp "$BIN_DIR/arbor" "dist/arbor-$V-$OS-$ARCH"
    STAGE="$(mktemp -d)/arbor-$V"
    mkdir -p "$STAGE"
    cp "$BIN_DIR/arbor" packaging/arbor.desktop assets/icon-256.png LICENSE "$STAGE/"
    mv "$STAGE/icon-256.png" "$STAGE/arbor.png"
    tar -C "$(dirname "$STAGE")" -czf "dist/arbor-$V-$OS-$ARCH.tar.gz" "arbor-$V"
    ;;
  Darwin)
    OS=macos
    cp "$BIN_DIR/arbor" "dist/arbor-$V-$OS-$ARCH"
    APP="$(mktemp -d)/arbor.app"
    mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
    cp "$BIN_DIR/arbor" "$APP/Contents/MacOS/arbor"
    sed "s/__VERSION__/$V/g" packaging/Info.plist > "$APP/Contents/Info.plist"
    ICONSET="$(mktemp -d)/arbor.iconset"
    mkdir -p "$ICONSET"
    for s in 16 32 128 256 512; do
      sips -z $s $s assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
      sips -z $((s * 2)) $((s * 2)) assets/icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/arbor.icns"
    codesign --force --deep --sign - "$APP" # assinatura ad hoc: sem ela o Apple Silicon recusa abrir
    (cd "$(dirname "$APP")" && ditto -c -k --keepParent arbor.app "$OLDPWD/dist/arbor-$V-$OS-$ARCH.zip")
    ;;
  MINGW*|MSYS*|CYGWIN*)
    OS=windows
    cp "$BIN_DIR/arbor.exe" "dist/arbor-$V-$OS-$ARCH.exe"
    STAGE="$(mktemp -d)"
    cp "$BIN_DIR/arbor.exe" LICENSE "$STAGE/"
    (cd "$STAGE" && 7z a -tzip -bso0 "arbor.zip" arbor.exe LICENSE)
    mv "$STAGE/arbor.zip" "dist/arbor-$V-$OS-$ARCH.zip"
    ;;
esac
ls -la dist
