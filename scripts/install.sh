#!/usr/bin/env bash
set -euo pipefail

REPO="tilesprivacy/tiles"

VERSION="0.4.15"
DEV="false"
NIGHTLY="false"

log() { echo -e "\033[1;36m$*\033[0m"; }
err() { echo -e "\033[1;31m$*\033[0m" >&2; exit 1; }
warn() {
  printf "\033[1;33m%s\033[0m\n" "$*"
}

usage() {
  echo "Usage: scripts/install.sh [--dev] [--nightly]"
  echo ""
  echo "  --dev       Install from a local dist/*.tar.gz instead of GitHub"
  echo "  --nightly   Install the latest nightly GitHub release"
  echo "              (e.g. tiles-v0.4.16-nightly.1-x86_64-linux.tar.gz)"
}

# Resolve the newest GitHub nightly release that has a tarball for this platform.
# Expected asset: tiles-v0.4.16-nightly.1-x86_64-linux.tar.gz
# Sets RELEASE_TAG (download URL) and VERSION (asset name).
resolve_nightly_version() {
  local api_url="https://api.github.com/repos/${REPO}/releases?per_page=30"
  local releases_json tag version asset
  local platform="${ARCH}-${OS}"

  releases_json="$(curl -fsSL "${api_url}")" || err "Failed to query GitHub releases for ${REPO}."

  # Walk nightly tags (API returns newest first) and pick the first with our asset.
  while IFS= read -r tag; do
    [[ -z "${tag}" ]] && continue
    version="${tag#v}"
    asset="tiles-v${version}-${platform}.tar.gz"
    if printf '%s' "${releases_json}" | grep -qF "\"name\": \"${asset}\""; then
      RELEASE_TAG="${tag}"
      VERSION="${version}"
      return 0
    fi
  done < <(
    printf '%s' "${releases_json}" \
      | grep -oE '"tag_name"[[:space:]]*:[[:space:]]*"[^"]+"' \
      | sed -E 's/.*"([^"]+)".*/\1/' \
      | grep -i 'nightly' \
      || true
  )

  err "No nightly found for ${platform} (no matching tiles-v*-${platform}.tar.gz on GitHub)."
}

for arg in "$@"; do
  case "$arg" in
    --dev|-dev)
      DEV="true"
      ;;
    --nightly|-nightly)
      NIGHTLY="true"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "${DEV}" == "true" && "${NIGHTLY}" == "true" ]]; then
  err "--dev and --nightly cannot be used together."
fi

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

if [[ "${OS}" == "linux" && "$(id -u)" != "0" ]]; then
  INSTALL_DIR="${HOME}/.local/bin"
  LIB_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/tiles"
else
  INSTALL_DIR="/usr/local/bin"
  LIB_DIR="/usr/local/share/tiles"
fi

SERVER_DIR="${LIB_DIR}/server"         # Python server folder
MODELFILE_DIR="${LIB_DIR}/modelfiles"  # Modelfile server folder
PI_DIR="${LIB_DIR}/pi"

TMPDIR="$(mktemp -d)"
RELEASE_TAG="${VERSION}"

if [[ "${NIGHTLY}" == "true" ]]; then
  resolve_nightly_version
  log "⬇️  Downloading Tiles nightly (${VERSION}) for ${ARCH}-${OS}..."
elif [[ "${DEV}" == "true" ]]; then
  log "⬇️  Installing local Tiles build for ${ARCH}-${OS}..."
else
  log "⬇️  Downloading Tiles (${VERSION}) for ${ARCH}-${OS}..."
fi

if [[ "${DEV}" == "false" ]]; then
  # Stable (default) and --nightly both download from GitHub.
  # Unflagged install always uses the hardcoded VERSION above (never nightly).
  # Nightly only runs when --nightly is passed (resolve_nightly_version).
  TAR_URL="https://github.com/${REPO}/releases/download/${RELEASE_TAG}/tiles-v${VERSION}-${ARCH}-${OS}.tar.gz"
  curl -fL -o "${TMPDIR}/tiles.tar.gz" "$TAR_URL"
else
  if [[ -n "${TILES_DEV_TARBALL:-}" ]]; then
    LOCAL_TARBALL="${TILES_DEV_TARBALL}"
  else
    LOCAL_TARBALL=""
    shopt -s nullglob
    LOCAL_TARBALLS=(dist/tiles-v*-"${ARCH}"-"${OS}".tar.gz)
    shopt -u nullglob
    for candidate in "${LOCAL_TARBALLS[@]}"; do
      if [[ -z "${LOCAL_TARBALL}" || "${candidate}" -nt "${LOCAL_TARBALL}" ]]; then
        LOCAL_TARBALL="${candidate}"
      fi
    done
  fi

  if [[ -z "${LOCAL_TARBALL}" || ! -f "${LOCAL_TARBALL}" ]]; then
    err "No local Tiles tarball found. Run just bundle first, or set TILES_DEV_TARBALL=/path/to/tar.gz."
  fi

  log "📦 Using local Tiles tarball ${LOCAL_TARBALL}"
  cp "${LOCAL_TARBALL}" "${TMPDIR}/tiles.tar.gz"
fi

echo "⬇️ Installing tiles..."
# Lets point to tile repo
tar -xzf "${TMPDIR}/tiles.tar.gz" -C "${TMPDIR}"

log "📦 Installing tiles binary to ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}"
install -m 755 "${TMPDIR}/tiles" "${INSTALL_DIR}/tiles"

log "Unpacking libs ..."
rm -rf "${MODELFILE_DIR}"

mkdir -p "${MODELFILE_DIR}"

cp -r "${TMPDIR}/modelfiles"/* "${MODELFILE_DIR}/"


log "Installing PI artifacts ..."

rm -rf "${PI_DIR}"

mkdir -p "${PI_DIR}"

cp -r "${TMPDIR}/pi"/* "${PI_DIR}/"


log "📦 Installing Python server to ${SERVER_DIR}..."
rm -rf "${SERVER_DIR}"

mkdir -p "${SERVER_DIR}"

cp -r "${TMPDIR}/server"/* "${SERVER_DIR}/"

chmod +x "${SERVER_DIR}/bin/llama-server" 2>/dev/null || true

log "🔧 Setting up Python environment..."
cd "${SERVER_DIR}/stack_export_prod"

for f in *.tar.xz; do
  tar -xvf "$f"
done

rm -rf *.tar.xz

cpython3.13/bin/python cpython3.13/postinstall.py
app-server/bin/python app-server/postinstall.py

rm -rf "${TMPDIR}"

log "✅ Tiles installed successfully!"
log ""

case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    echo "🚀 Start Tiles by running \"tiles\""
    ;;
  *)
    echo ""
    warn "⚠️  $INSTALL_DIR is not in your PATH."
    echo ""
    echo "ℹ️  To use Tiles, add this line to your shell configuration(ex: ~/.bashrc, ~/.zshrc)"
    echo ""
    echo "  export PATH=$INSTALL_DIR:\$PATH"
    echo ""
    echo "🚀 Then restart your terminal..."
    ;;
esac
