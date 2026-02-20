#!/usr/bin/env bash

set -euo pipefail

REPO_OWNER="SKKUGoon"
REPO_NAME="Raven"
TARGET="x86_64-unknown-linux-gnu"
DOWNLOAD_DIR="${HOME}/downloads"

if [[ -t 1 ]]; then
  C_RESET="$(printf '\033[0m')"
  C_BOLD="$(printf '\033[1m')"
  C_BLUE="$(printf '\033[34m')"
  C_GREEN="$(printf '\033[32m')"
  C_YELLOW="$(printf '\033[33m')"
  C_RED="$(printf '\033[31m')"
else
  C_RESET=""
  C_BOLD=""
  C_BLUE=""
  C_GREEN=""
  C_YELLOW=""
  C_RED=""
fi

info() { echo -e "${C_BLUE}${1}${C_RESET}"; }
warn() { echo -e "${C_YELLOW}${1}${C_RESET}"; }
success() { echo -e "${C_GREEN}${1}${C_RESET}"; }
error() { echo -e "${C_RED}${1}${C_RESET}" >&2; }

echo -e "${C_BOLD}Raven GitHub Release Deployer${C_RESET}"
info "Target: ${TARGET}"
echo

read -r -p "Enter Raven version (example: 2.5.3): " VERSION

if [[ -z "${VERSION}" ]]; then
  error "Version is required."
  exit 1
fi

if [[ ! "${VERSION}" =~ ^[0-9]+(\.[0-9]+){1,2}([.-][A-Za-z0-9]+)?$ ]]; then
  error "Invalid version format: ${VERSION}"
  warn "Use versions like: 2.5.3"
  exit 1
fi

ARCHIVE_NAME="raven-v${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/v${VERSION}/${ARCHIVE_NAME}"

mkdir -p "${DOWNLOAD_DIR}"
cd "${DOWNLOAD_DIR}"

echo
info "Downloading: ${DOWNLOAD_URL}"
curl -fL -o "${ARCHIVE_NAME}" "${DOWNLOAD_URL}"

info "Extracting: ${ARCHIVE_NAME}"
EXTRACT_DIR="$(mktemp -d)"
tar -xzf "${ARCHIVE_NAME}" -C "${EXTRACT_DIR}"

# Support both archive layouts:
# 1) binaries directly at root
# 2) a single top-level version directory containing binaries
INSTALL_SOURCE="${EXTRACT_DIR}"
shopt -s nullglob
subdirs=("${EXTRACT_DIR}"/*/)
shopt -u nullglob
if [[ "${#subdirs[@]}" -eq 1 ]]; then
  INSTALL_SOURCE="${subdirs[0]%/}"
fi

echo
warn "Installing executables to /usr/local/bin (sudo required)..."

installed_count=0
for f in "${INSTALL_SOURCE}"/*; do
  if [[ -f "${f}" && -x "${f}" ]]; then
    sudo install -m 0755 "${f}" /usr/local/bin/
    ((installed_count += 1))
  fi
done

if [[ "${installed_count}" -eq 0 ]]; then
  error "No executable files found in extracted archive."
  exit 1
fi

info "Cleaning up downloaded files..."
rm -f "${ARCHIVE_NAME}"
rm -rf "${EXTRACT_DIR}"

echo
success "Installed ${installed_count} binary file(s) into /usr/local/bin."
success "Done."
