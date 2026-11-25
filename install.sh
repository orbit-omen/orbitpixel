#!/usr/bin/env sh
set -eu

REPO="orbit-omen/orbitpixel"
BIN_NAME="orbitpixel"
INSTALL_DIR="${ORBITPIXEL_INSTALL_DIR:-$HOME/.local/bin}"

log() {
    printf '[orbitpixel] %s\n' "$1"
}

err() {
    printf '[orbitpixel] ERROR: %s\n' "$1" >&2
    exit 1
}

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux) os_part="unknown-linux-gnu" ;;
        Darwin) os_part="apple-darwin" ;;
        *) err "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64) arch_part="x86_64" ;;
        aarch64|arm64) arch_part="aarch64" ;;
        *) err "Unsupported CPU architecture: $arch" ;;
    esac

    printf '%s-%s' "$arch_part" "$os_part"
}

verify_sha256() {
    file="$1"
    expected="$2"
    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$file" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
        actual="$(shasum -a 256 "$file" | awk '{print $1}')"
    else
        log "No sha256sum/shasum found; skipping checksum verification."
        return 0
    fi

    if [ "$actual" != "$expected" ]; then
        err "Checksum mismatch: expected $expected, got $actual"
    fi
}

install_from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        err "No prebuilt release found and 'cargo' is not installed. Install Rust via https://rustup.rs and re-run."
    fi
    log "Building from source via cargo install..."
    cargo install --git "https://github.com/${REPO}" --branch main --locked || \
        cargo install --git "https://github.com/${REPO}" --branch main
    log "Installed ${BIN_NAME} into Cargo bin directory."
}

main() {
    target="$(detect_target)"
    log "Detected platform: ${target}"

    mkdir -p "$INSTALL_DIR"
    tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t orbitpixel)"
    trap 'rm -rf "$tmp_dir"' EXIT

    api_url="https://api.github.com/repos/${REPO}/releases/latest"
    tag=""
    if command -v curl >/dev/null 2>&1; then
        tag="$(curl -fsSL "$api_url" 2>/dev/null | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 || true)"
    elif command -v wget >/dev/null 2>&1; then
        tag="$(wget -qO- "$api_url" 2>/dev/null | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 || true)"
    else
        err "Either 'curl' or 'wget' is required."
    fi

    if [ -z "$tag" ]; then
        log "No published GitHub release found yet."
        install_from_source
        exit 0
    fi

    archive="${BIN_NAME}-${target}.tar.gz"
    base_url="https://github.com/${REPO}/releases/download/${tag}"
    archive_path="${tmp_dir}/${archive}"
    sha_path="${tmp_dir}/${archive}.sha256"

    log "Downloading ${BIN_NAME} ${tag} (${target})..."
    if command -v curl >/dev/null 2>&1; then
        if ! curl -fsSL "${base_url}/${archive}" -o "$archive_path"; then
            log "Prebuilt archive unavailable for ${target}."
            install_from_source
            exit 0
        fi
        if curl -fsSL "${base_url}/${archive}.sha256" -o "$sha_path" 2>/dev/null; then
            expected_sha="$(awk '{print $1}' "$sha_path")"
            if [ -n "$expected_sha" ]; then
                verify_sha256 "$archive_path" "$expected_sha"
            fi
        fi
    else
        if ! wget -qO "$archive_path" "${base_url}/${archive}"; then
            log "Prebuilt archive unavailable for ${target}."
            install_from_source
            exit 0
        fi
    fi

    tar -xzf "$archive_path" -C "$tmp_dir"
    if [ ! -f "${tmp_dir}/${BIN_NAME}" ]; then
        err "Archive did not contain expected binary '${BIN_NAME}'."
    fi

    chmod +x "${tmp_dir}/${BIN_NAME}"
    mv "${tmp_dir}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
    log "Successfully installed ${BIN_NAME} to ${INSTALL_DIR}/${BIN_NAME}"

    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            log "Add ${INSTALL_DIR} to your PATH to run '${BIN_NAME}' globally:"
            printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR"
            ;;
    esac
}

main "$@"
