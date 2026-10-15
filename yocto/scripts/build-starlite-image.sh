#!/usr/bin/env bash
set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly REPOSITORY_ROOT
readonly LATTICE_YOCTO_WORKDIR="${LATTICE_YOCTO_WORKDIR:-${HOME}/.cache/lattice-yocto}"
readonly POKY="${LATTICE_YOCTO_WORKDIR}/sources/poky"
readonly META_OPENEMBEDDED="${LATTICE_YOCTO_WORKDIR}/sources/meta-openembedded"
readonly BUILD_DIR="${LATTICE_YOCTO_WORKDIR}/build-starlite"
readonly ARTIFACT_DIR="${LATTICE_YOCTO_WORKDIR}/artifacts/starlite"
readonly LAYER="${REPOSITORY_ROOT}/yocto/meta-lattice-tablet"
# shellcheck source=../sources.conf
source "${REPOSITORY_ROOT}/yocto/sources.conf"
prepare_only=false
case "${1:-}" in
    --prepare-only) prepare_only=true ;;
    "") ;;
    *) printf 'Usage: %s [--prepare-only]\n' "$0" >&2; exit 1 ;;
esac

if ! $prepare_only && [[ -r /proc/sys/kernel/apparmor_restrict_unprivileged_userns ]] &&
    [[ "$(</proc/sys/kernel/apparmor_restrict_unprivileged_userns)" != "0" ]]; then
    cat >&2 <<'EOF'
BitBake user namespaces are blocked by this host's AppArmor policy.
Temporarily enable them for the build, then restore the restriction:

  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
  yocto/scripts/build-starlite-image.sh
  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=1
EOF
    exit 1
fi

require_revision() {
    local repository="$1"
    local expected="$2"
    local actual
    if [[ ! -d "${repository}/.git" ]]; then
        printf 'missing Yocto source checkout: %s\n' "${repository}" >&2
        printf 'Run yocto/scripts/prepare-sources.sh first.\n' >&2
        exit 1
    fi
    actual="$(git -C "${repository}" rev-parse HEAD)"
    if [[ "${actual}" != "${expected}" ]]; then
        printf 'unexpected source revision for %s: %s (expected %s)\n' \
            "${repository}" "${actual}" "${expected}" >&2
        exit 1
    fi
    if [[ -n "$(git -C "${repository}" status --porcelain)" ]]; then
        printf 'Yocto source checkout has local changes: %s\n' "${repository}" >&2
        exit 1
    fi
}

require_revision "${POKY}" "${EXPECTED_POKY_REVISION}"
require_revision "${META_OPENEMBEDDED}" "${EXPECTED_META_OPENEMBEDDED_REVISION}"

mkdir -p "${ARTIFACT_DIR}" "${BUILD_DIR}/conf"

cargo build \
    --manifest-path "${REPOSITORY_ROOT}/Cargo.toml" \
    --target-dir "${REPOSITORY_ROOT}/target" \
    --locked \
    --release \
    --target x86_64-unknown-linux-gnu \
    --no-default-features \
    --features wayland,gpu

install -m0755 \
    "${REPOSITORY_ROOT}/target/x86_64-unknown-linux-gnu/release/lattice-tablet" \
    "${ARTIFACT_DIR}/lattice-tablet"
cargo build \
    --manifest-path "${REPOSITORY_ROOT}/peer/Cargo.toml" \
    --target-dir "${REPOSITORY_ROOT}/peer/target" \
    --locked --release --target x86_64-unknown-linux-gnu
install -m0755 \
    "${REPOSITORY_ROOT}/peer/target/x86_64-unknown-linux-gnu/release/lattice-peer" \
    "${ARTIFACT_DIR}/lattice-peer"

if [[ -n ${LATTICE_SSH_AUTHORIZED_KEYS:-} ]]; then
    install -m0644 "${LATTICE_SSH_AUTHORIZED_KEYS}" "${ARTIFACT_DIR}/authorized_keys"
else
    # An image without explicitly supplied public keys has no remote login key.
    : > "${ARTIFACT_DIR}/authorized_keys"
fi

sed \
    -e "s|@POKY@|${POKY}|g" \
    -e "s|@META_OPENEMBEDDED@|${META_OPENEMBEDDED}|g" \
    -e "s|@LAYER@|${LAYER}|g" \
    "${REPOSITORY_ROOT}/yocto/conf/bblayers.conf.in" \
    > "${BUILD_DIR}/conf/bblayers.conf"
sed \
    -e "s|@WORKDIR@|${LATTICE_YOCTO_WORKDIR}|g" \
    -e "s|@ARTIFACT_DIR@|${ARTIFACT_DIR}|g" \
    "${REPOSITORY_ROOT}/yocto/conf/local.conf.in" \
    > "${BUILD_DIR}/conf/local.conf"

if $prepare_only; then
    printf 'Prepared application and image configuration in %s\n' "${BUILD_DIR}"
    exit 0
fi

export TEMPLATECONF="${POKY}/meta-poky/conf/templates/default"
set +u
# shellcheck disable=SC1091
source "${POKY}/oe-init-build-env" "${BUILD_DIR}" >/dev/null
set -u
bitbake lattice-tablet-image

printf '\nImage artifacts:\n'
for extension in wic.zst wic.bmap; do
    artifact="${BUILD_DIR}/tmp/deploy/images/genericx86-64/lattice-tablet-image-genericx86-64.rootfs.${extension}"
    printf '%s -> %s\n' "${artifact}" "$(readlink -f "${artifact}")"
done
