#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage:
  install-starlite-internal.sh \
    (--image IMAGE | --source-esp PARTITION --source-root PARTITION) \
    --esp PARTITION --root-a PARTITION [--root-b PARTITION] \
    --state PARTITION (--fresh-state | --migrate-state PARTITION) [--yes]

Installs Lattice into explicit, already-created partitions. This script never
creates, moves, resizes, or deletes partitions and never writes a whole disk.
EOF
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

image=""
source_esp=""
source_root=""
destination_esp=""
destination_root=""
destination_root_b=""
destination_state=""
migrate_state=""
fresh_state=false
confirmed=false

while (($#)); do
    case "$1" in
        --image|--source-esp|--source-root|--esp|--root-a|--root-b|--state|--migrate-state)
            (($# >= 2)) || fail "$1 requires a value"
            option="$1"
            value="$2"
            shift 2
            case "$option" in
                --image) image="$value" ;;
                --source-esp) source_esp="$value" ;;
                --source-root) source_root="$value" ;;
                --esp) destination_esp="$value" ;;
                --root-a) destination_root="$value" ;;
                --root-b) destination_root_b="$value" ;;
                --state) destination_state="$value" ;;
                --migrate-state) migrate_state="$value" ;;
            esac
            ;;
        --fresh-state)
            fresh_state=true
            shift
            ;;
        --yes)
            confirmed=true
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *) fail "unknown argument: $1" ;;
    esac
done

[[ $EUID -eq 0 ]] || fail "run as root"
for command in blkid blockdev cp find findmnt grep lsblk mktemp mount mountpoint readlink sed sync umount; do
    command -v "$command" >/dev/null || fail "required command is missing: $command"
done

[[ -n $destination_esp && -n $destination_root && -n $destination_state ]] || {
    usage >&2
    fail "--esp, --root-a, and --state are required"
}
if [[ -n $image ]]; then
    [[ -z $source_esp && -z $source_root ]] || fail "--image cannot be combined with source partitions"
    [[ -f $image ]] || fail "image does not exist: $image"
    command -v losetup >/dev/null || fail "losetup is required for --image"
else
    [[ -n $source_esp && -n $source_root ]] || fail "provide --image or both source partitions"
fi
if $fresh_state; then
    [[ -z $migrate_state ]] || fail "choose either --fresh-state or --migrate-state"
else
    [[ -n $migrate_state ]] || fail "choose either --fresh-state or --migrate-state"
fi

loop_device=""
mount_root=""
mounted=()
cleanup() {
    set +e
    for ((index=${#mounted[@]} - 1; index >= 0; index--)); do
        mountpoint -q "${mounted[index]}" && umount "${mounted[index]}"
    done
    [[ -z $loop_device ]] || losetup -d "$loop_device"
    [[ -z $mount_root ]] || rmdir "$mount_root"/* "$mount_root" 2>/dev/null
}
trap cleanup EXIT INT TERM

if [[ -n $image ]]; then
    loop_device="$(losetup --find --show --partscan --read-only "$image")"
    command -v udevadm >/dev/null && udevadm settle
    source_esp="${loop_device}p1"
    source_root="${loop_device}p2"
    [[ -b $source_esp && -b $source_root ]] || fail "image does not expose boot and root partitions"
fi

canonical_partition() {
    local requested="$1"
    local resolved name
    resolved="$(readlink -f "$requested")"
    [[ -b $resolved ]] || fail "not a block device: $requested"
    name="${resolved##*/}"
    [[ -f "/sys/class/block/$name/partition" ]] || fail "whole-disk target is forbidden: $requested"
    printf '%s\n' "$resolved"
}

source_esp="$(canonical_partition "$source_esp")"
source_root="$(canonical_partition "$source_root")"
destination_esp="$(canonical_partition "$destination_esp")"
destination_root="$(canonical_partition "$destination_root")"
destination_state="$(canonical_partition "$destination_state")"
[[ -z $destination_root_b ]] || destination_root_b="$(canonical_partition "$destination_root_b")"
[[ -z $migrate_state ]] || migrate_state="$(canonical_partition "$migrate_state")"

destinations=("$destination_esp" "$destination_root" "$destination_state")
[[ -z $destination_root_b ]] || destinations+=("$destination_root_b")
sources=("$source_esp" "$source_root")
[[ -z $migrate_state ]] || sources+=("$migrate_state")

for source in "${sources[@]}"; do
    findmnt -rn -S "$source" >/dev/null && fail "source must be offline: $source"
done
for destination in "${destinations[@]}"; do
    findmnt -rn -S "$destination" >/dev/null && fail "destination is mounted: $destination"
    for source in "${sources[@]}"; do
        [[ $destination != "$source" ]] || fail "source and destination are identical: $source"
    done
done

[[ $(blockdev --getsize64 "$destination_esp") -ge $(blockdev --getsize64 "$source_esp") ]] || {
    fail "destination ESP is smaller than its source"
}
[[ $(blockdev --getsize64 "$destination_root") -ge $(blockdev --getsize64 "$source_root") ]] || {
    fail "destination root A is smaller than its source"
}
if [[ -n $migrate_state ]]; then
    [[ $(blockdev --getsize64 "$destination_state") -ge $(blockdev --getsize64 "$migrate_state") ]] || {
        fail "destination state is smaller than its source"
    }
fi
for ((left=0; left < ${#destinations[@]}; left++)); do
    for ((right=left + 1; right < ${#destinations[@]}; right++)); do
        [[ ${destinations[left]} != "${destinations[right]}" ]] || fail "duplicate destination partition"
    done
done

filesystem_type() {
    blkid -s TYPE -o value "$1"
}
filesystem_label() {
    blkid -s LABEL -o value "$1"
}
require_filesystem() {
    local device="$1" expected="$2" role="$3"
    [[ $(filesystem_type "$device") == "$expected" ]] || fail "$role must use $expected: $device"
}

require_filesystem "$source_esp" vfat "source ESP"
require_filesystem "$source_root" ext4 "source root"
require_filesystem "$destination_esp" vfat "destination ESP"
require_filesystem "$destination_root" ext4 "destination root A"
require_filesystem "$destination_state" ext4 "destination state"
[[ -z $destination_root_b ]] || require_filesystem "$destination_root_b" ext4 "destination root B"
[[ -z $migrate_state ]] || require_filesystem "$migrate_state" ext4 "state source"

[[ $(filesystem_label "$source_esp") == LATTICEBOOT ]] || fail "source ESP is not LATTICEBOOT"
[[ $(filesystem_label "$source_root") == lattice-root ]] || fail "source root is not lattice-root"
[[ $(filesystem_label "$destination_esp") == LATTICEBOOT ]] || fail "destination ESP is not LATTICEBOOT"
[[ $(filesystem_label "$destination_root") == lattice-root-A ]] || fail "destination root is not lattice-root-A"
[[ $(filesystem_label "$destination_state") == lattice-state ]] || fail "destination state is not lattice-state"
[[ -z $destination_root_b || $(filesystem_label "$destination_root_b") == lattice-root-B ]] || {
    fail "destination root B is not lattice-root-B"
}
[[ -z $migrate_state || $(filesystem_label "$migrate_state") == lattice-state ]] || {
    fail "state source is not lattice-state"
}

destination_parent="$(lsblk -ndo PKNAME "$destination_esp")"
for destination in "${destinations[@]}"; do
    [[ $(lsblk -ndo PKNAME "$destination") == "$destination_parent" ]] || {
        fail "all destination partitions must share one disk"
    }
done

printf '%s\n' \
    "Lattice internal installation" \
    "  source ESP:       $source_esp" \
    "  source root:      $source_root" \
    "  destination disk: /dev/$destination_parent" \
    "  destination ESP:  $destination_esp" \
    "  destination A:    $destination_root" \
    "  destination B:    ${destination_root_b:-not supplied}" \
    "  destination state:$destination_state" \
    "  state mode:        $([[ -n $migrate_state ]] && printf 'migrate from %s' "$migrate_state" || printf fresh)"

if ! $confirmed; then
    printf 'Type INSTALL to overwrite the explicit destination filesystems: '
    read -r answer
    [[ $answer == INSTALL ]] || fail "installation cancelled"
fi

mount_root="$(mktemp -d /tmp/lattice-internal-install.XXXXXX)"
mkdir "$mount_root"/{source-esp,source-root,source-state,destination-esp,destination-root,destination-state}

mount -o ro "$source_esp" "$mount_root/source-esp"
mounted+=("$mount_root/source-esp")
mount -o ro,noload "$source_root" "$mount_root/source-root"
mounted+=("$mount_root/source-root")
if [[ -n $migrate_state ]]; then
    mount -o ro,noload "$migrate_state" "$mount_root/source-state"
    mounted+=("$mount_root/source-state")
fi
mount "$destination_esp" "$mount_root/destination-esp"
mounted+=("$mount_root/destination-esp")
mount "$destination_root" "$mount_root/destination-root"
mounted+=("$mount_root/destination-root")
mount "$destination_state" "$mount_root/destination-state"
mounted+=("$mount_root/destination-state")

for destination in destination-esp destination-root destination-state; do
    [[ -z $(find "$mount_root/$destination" -mindepth 1 -maxdepth 1 ! -name lost+found -print -quit) ]] || {
        fail "$destination filesystem is not empty"
    }
done

cp -a "$mount_root/source-esp/." "$mount_root/destination-esp/"
cp -a "$mount_root/source-root/." "$mount_root/destination-root/"
if [[ -n $migrate_state ]]; then
    cp -a "$mount_root/source-state/." "$mount_root/destination-state/"
fi

esp_uuid="$(blkid -s UUID -o value "$destination_esp")"
state_uuid="$(blkid -s UUID -o value "$destination_state")"
root_partuuid="$(blkid -s PARTUUID -o value "$destination_root")"
fstab="$mount_root/destination-root/etc/fstab"
boot_entry="$mount_root/destination-esp/loader/entries/boot.conf"
[[ -f $fstab ]] || fail "installed root has no /etc/fstab"
[[ -f $boot_entry ]] || fail "installed ESP has no loader entry"

sed -i -E \
    -e "s#^UUID=[^[:space:]]+[[:space:]]+/boot[[:space:]].*#UUID=$esp_uuid\t/boot\tvfat\tdefaults\t0\t0#" \
    -e "s#^UUID=[^[:space:]]+[[:space:]]+/var/lib[[:space:]].*#UUID=$state_uuid\t/var/lib\text4\tdefaults\t0\t0#" \
    "$fstab"
sed -i -E \
    -e "s#root=PARTUUID=[^[:space:]]+#root=PARTUUID=$root_partuuid#" \
    -e '/^options /{/lattice\.slot=/!s/$/ lattice.slot=A/;}' \
    "$boot_entry"

grep -q "^UUID=$esp_uuid[[:space:]]\+/boot" "$fstab" || fail "boot UUID rewrite failed"
grep -q "^UUID=$state_uuid[[:space:]]\+/var/lib" "$fstab" || fail "state UUID rewrite failed"
grep -q "root=PARTUUID=$root_partuuid" "$boot_entry" || fail "root PARTUUID rewrite failed"
grep -q 'lattice.slot=A' "$boot_entry" || fail "slot identity rewrite failed"
[[ -x $mount_root/destination-root/usr/bin/lattice-tablet ]] || fail "Lattice binary is missing"
[[ -f $mount_root/destination-esp/EFI/BOOT/bootx64.efi ]] || fail "EFI loader is missing"

sync
printf '%s\n' \
    "Installation complete and verified." \
    "  ESP UUID:      $esp_uuid" \
    "  root PARTUUID: $root_partuuid" \
    "  state UUID:    $state_uuid" \
    "Register the independent Lattice UEFI entry only after inspecting these filesystems offline."
