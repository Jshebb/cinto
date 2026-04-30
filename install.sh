#!/bin/sh
set -eu

repo="${CINTO_INSTALL_REPO:-joaoh/cinto}"
version="${CINTO_INSTALL_VERSION:-latest}"
bin_dir="${CINTO_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}/cinto"

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

verify_archive_shape() {
  archive="$1"
  tar -tzf "$archive" | while IFS= read -r entry; do
    case "$entry" in
      cinto|cinto.exe) ;;
      *) fail "release archive contains unexpected path: $entry" ;;
    esac
  done
}

verify_checksum() {
  archive="$1"
  checksum_file="$2"

  if [ "${CINTO_INSTALL_SKIP_CHECKSUM:-}" = "1" ]; then
    say "Skipping checksum verification because CINTO_INSTALL_SKIP_CHECKSUM=1."
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$archive")" && sha256sum -c "$(basename "$checksum_file")")
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$(dirname "$archive")" && shasum -a 256 -c "$(basename "$checksum_file")")
  else
    fail "missing sha256sum or shasum for checksum verification"
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
download "$url.sha256" "$archive.sha256"
verify_checksum "$archive" "$archive.sha256"
verify_archive_shape "$archive"

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

mkdir -p "$data_dir"
{
  printf 'binary=%s\n' "$bin_dir/$installed"
  printf 'repo=%s\n' "$repo"
  printf 'version=%s\n' "$version"
  printf 'target=%s\n' "$target"
} > "$data_dir/install.toml"

say "Installed cinto to $bin_dir/$installed"
say "Uninstall with: cinto uninstall"

case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *)
    profile=""
    case "$(basename "${SHELL:-/bin/sh}")" in
      zsh)
        profile="$HOME/.zshrc"
        ;;
      bash)
        profile="$HOME/.bashrc"
        ;;
      *)
        if [ -f "$HOME/.zshrc" ]; then
          profile="$HOME/.zshrc"
        elif [ -f "$HOME/.bashrc" ]; then
          profile="$HOME/.bashrc"
        elif [ -f "$HOME/.profile" ]; then
          profile="$HOME/.profile"
        fi
        ;;
    esac

    if [ -n "$profile" ]; then
      touch "$profile" 2>/dev/null || true
      if grep -q "export PATH=.*$bin_dir" "$profile" 2>/dev/null; then
        say "Warning: $bin_dir is not on current PATH but was found in $profile."
        say "Restart your terminal or run: source $profile"
      else
        printf '\n# cinto path\nexport PATH="%s:$PATH"\n' "$bin_dir" >> "$profile"
        say "Added $bin_dir to PATH in $profile"
        say "Restart your terminal or run: source $profile"
      fi
    else
      say "Warning: $bin_dir is not on PATH."
      say "Add it to your shell profile, then run: cinto"
    fi
    ;;
esac
