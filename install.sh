#!/bin/sh
set -eu

repo="${CINTO_INSTALL_REPO:-joaoh/cinto}"
version="${CINTO_INSTALL_VERSION:-latest}"
bin_dir="${CINTO_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"

say() {
  printf '%s\n' "$1"
}

fail() {
  say "cinto install: $1" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) target="x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) target="aarch64-unknown-linux-gnu" ;;
        *) fail "unsupported Linux architecture: $arch" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64|amd64) target="x86_64-apple-darwin" ;;
        aarch64|arm64) target="aarch64-apple-darwin" ;;
        *) fail "unsupported macOS architecture: $arch" ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*)
      case "$arch" in
        x86_64|amd64) target="x86_64-pc-windows-msvc" ;;
        *) fail "unsupported Windows architecture: $arch" ;;
      esac
      ;;
    *)
      fail "unsupported OS: $os"
      ;;
  esac

  printf '%s' "$target"
}

download() {
  url="$1"
  out="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$out" "$url"
  else
    fail "missing curl or wget"
  fi
}

target="$(detect_target)"
asset="cinto-$target.tar.gz"

if [ "$version" = "latest" ]; then
  url="https://github.com/$repo/releases/latest/download/$asset"
else
  url="https://github.com/$repo/releases/download/$version/$asset"
fi

need_cmd tar
tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t cinto-install)"
trap 'rm -rf "$tmp_dir"' EXIT INT TERM

archive="$tmp_dir/$asset"
say "Downloading cinto for $target..."
download "$url" "$archive"

mkdir -p "$tmp_dir/extract"
tar -xzf "$archive" -C "$tmp_dir/extract"

binary="$tmp_dir/extract/cinto"
installed="cinto"
if [ ! -f "$binary" ] && [ -f "$tmp_dir/extract/cinto.exe" ]; then
  binary="$tmp_dir/extract/cinto.exe"
  installed="cinto.exe"
fi
[ -f "$binary" ] || fail "release archive did not contain cinto"

mkdir -p "$bin_dir"
if command -v install >/dev/null 2>&1; then
  install -m 755 "$binary" "$bin_dir/$installed"
else
  cp "$binary" "$bin_dir/$installed"
  chmod 755 "$bin_dir/$installed"
fi

say "Installed cinto to $bin_dir/$installed"
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *)
    say "Warning: $bin_dir is not on PATH."
    say "Add it to your shell profile, then run: cinto"
    ;;
esac
