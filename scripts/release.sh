#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

VERSION="${EDGEL_VERSION:-v0.1.0}"
TARGETS="${EDGEL_TARGETS:-$(rustc -vV | sed -n 's/^host: //p')}"
RELEASE_DIR="dist/releases/${VERSION}"

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing required command: $1" >&2
    exit 1
  }
}

checksum() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  else
    shasum -a 256 "$file" | awk '{print $1}'
  fi
}

package_target() {
  local target="$1"
  local bin_ext="" archive_name archive_path stage_dir

  if [[ "$target" == *windows* ]]; then
    bin_ext=".exe"
  fi

  echo "Building ${target}" >&2
  cargo build --release -p edgel -p goldedge-browser --target "$target"

  stage_dir="${RELEASE_DIR}/edgel-${VERSION}-${target}"
  rm -rf "$stage_dir"
  mkdir -p "${stage_dir}/bin" "${stage_dir}/docs"

  cp "target/${target}/release/edgel${bin_ext}" "${stage_dir}/bin/edgel${bin_ext}"
  cp "target/${target}/release/goldedge-browser${bin_ext}" "${stage_dir}/bin/goldedge-browser${bin_ext}"
  cp README.md "${stage_dir}/README.md"
  cp docs/install.md "${stage_dir}/docs/install.md"
  cp docs/release-notes-v0.1.0.md "${stage_dir}/docs/release-notes-v0.1.0.md"

  if [[ "$target" == *windows* ]]; then
    archive_name="edgel-${VERSION}-${target}.zip"
    archive_path="${RELEASE_DIR}/${archive_name}"
    rm -f "$archive_path"
    (
      cd "${RELEASE_DIR}"
      if command -v zip >/dev/null 2>&1; then
        zip -rq "$archive_name" "edgel-${VERSION}-${target}"
      elif command -v 7z >/dev/null 2>&1; then
        7z a -tzip "$archive_name" "edgel-${VERSION}-${target}" >/dev/null
      else
        echo "Missing zip or 7z for Windows packaging" >&2
        exit 1
      fi
    )
  else
    archive_name="edgel-${VERSION}-${target}.tar.gz"
    archive_path="${RELEASE_DIR}/${archive_name}"
    rm -f "$archive_path"
    tar -czf "$archive_path" -C "${RELEASE_DIR}" "edgel-${VERSION}-${target}"
  fi

  printf '    {"target":"%s","archive":"%s","sha256":"%s"}' \
    "$target" "$archive_name" "$(checksum "$archive_path")"
}

main() {
  require cargo
  require rustc
  require tar
  mkdir -p "$RELEASE_DIR"

  IFS=' ' read -r -a target_list <<<"$TARGETS"
  if [[ "${#target_list[@]}" -eq 0 ]]; then
    echo "No targets configured" >&2
    exit 1
  fi

  local manifest_entries=()
  for target in "${target_list[@]}"; do
    manifest_entries+=("$(package_target "$target")")
  done

  cat > "${RELEASE_DIR}/manifest.json" <<EOF
{
  "version": "${VERSION}",
  "artifacts": [
$(printf '%s\n' "${manifest_entries[@]}" | sed 's/$/,/' | sed '$s/,$//')
  ]
}
EOF

  cp edgel.sh "${RELEASE_DIR}/edgel.sh"
  if [[ -f "edgel.ps1" ]]; then
    cp edgel.ps1 "${RELEASE_DIR}/edgel.ps1"
  fi

  echo "Release artifacts written to ${RELEASE_DIR}"
}

main "$@"
