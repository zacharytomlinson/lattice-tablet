#!/usr/bin/env bash
set -euo pipefail
readonly REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly LATTICE_YOCTO_WORKDIR="${LATTICE_YOCTO_WORKDIR:-${HOME}/.cache/lattice-yocto}"
# shellcheck source=../sources.conf
source "${REPOSITORY_ROOT}/yocto/sources.conf"

prepare() {
    local name="$1" url="$2" revision="$3"
    local destination="${LATTICE_YOCTO_WORKDIR}/sources/${name}"
    if [[ -e "$destination" ]]; then
        [[ -d "$destination/.git" ]] || { printf 'Not a Git checkout: %s\n' "$destination" >&2; exit 1; }
        [[ "$(git -C "$destination" rev-parse HEAD)" == "$revision" ]] || {
            printf 'Existing checkout has a different revision: %s\n' "$destination" >&2; exit 1;
        }
        [[ -z "$(git -C "$destination" status --porcelain)" ]] || {
            printf 'Existing checkout has local changes: %s\n' "$destination" >&2; exit 1;
        }
        return
    fi
    git init "$destination"
    git -C "$destination" remote add origin "$url"
    git -C "$destination" fetch --depth 1 origin "$revision"
    git -C "$destination" checkout --detach FETCH_HEAD
}
prepare poky https://git.yoctoproject.org/poky "$EXPECTED_POKY_REVISION"
prepare meta-openembedded https://git.openembedded.org/meta-openembedded "$EXPECTED_META_OPENEMBEDDED_REVISION"
