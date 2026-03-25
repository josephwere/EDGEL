#!/usr/bin/env bash
set -euo pipefail

VERSION="${EDGEL_VERSION:-v0.1.0}"
INSTALL_DIR="${EDGEL_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="${EDGEL_RELEASE_BASE_URL:-https://github.com/josephwere/EDGEL/releases/download/${VERSION}}"
TMPDIR_EDGEL=""

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

cleanup() {
  if [[ -n "${TMPDIR_EDGEL}" && -d "${TMPDIR_EDGEL}" ]]; then
    rm -rf "${TMPDIR_EDGEL}"
  fi
}

detect_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux) os="unknown-linux-gnu" ;;
    Darwin) os="apple-darwin" ;;
    *)
      echo "Unsupported operating system: $os" >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="aarch64" ;;
    *)
      echo "Unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf "%s-%s" "$arch" "$os"
}

main() {
  require curl
  require tar

  local target archive tmpdir stage_dir
  target="$(detect_target)"
  archive="edgel-${VERSION}-${target}.tar.gz"
  tmpdir="$(mktemp -d)"
  TMPDIR_EDGEL="$tmpdir"
  trap cleanup EXIT

  echo "Downloading EDGEL ${VERSION} for ${target}"
  curl -fsSL "${BASE_URL}/${archive}" -o "${tmpdir}/${archive}"
  tar -xzf "${tmpdir}/${archive}" -C "$tmpdir"

  stage_dir="${tmpdir}/edgel-${VERSION}-${target}"
  if [[ ! -d "$stage_dir" ]]; then
    echo "Release archive layout is invalid: ${stage_dir} missing" >&2
    exit 1
  fi

  mkdir -p "$INSTALL_DIR"
  install -m 0755 "${stage_dir}/bin/edgel" "${INSTALL_DIR}/edgel"
  install -m 0755 "${stage_dir}/bin/goldedge-browser" "${INSTALL_DIR}/goldedge-browser"

  echo "Installed:"
  echo " - ${INSTALL_DIR}/edgel"
  echo " - ${INSTALL_DIR}/goldedge-browser"

  case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      echo
      echo "Add ${INSTALL_DIR} to your PATH if needed:"
      echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
      ;;
  esac

  echo
  echo "Next steps:"
  echo "  edgel new my-app"
  echo "  cd my-app"
  echo "  edgel run"
}

main "$@"
