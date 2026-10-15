SUMMARY = "Lattice StarLite removable-media bring-up image"
LICENSE = "MIT"

SPLASH = "plymouth"

IMAGE_INSTALL = "\
    packagegroup-core-boot \
    lattice-power \
    lattice-tablet-bin \
    lattice-auto-rotate \
    weston-init \
    iwd \
    iw \
    linux-firmware-iwlwifi \
    linux-firmware-ibt-misc \
    linux-firmware-i915 \
    bluez5 \
    bluez5-noinst-tools \
    wireless-regdb-static \
    kernel-modules \
    dropbear \
    gptfdisk \
    parted \
    e2fsprogs-resize2fs \
    e2fsprogs-mke2fs \
    dosfstools \
    "

# Bring-up access is key-only: allow the root account through Dropbear while
# leaving its password locked. Remove this feature when production servicing
# credentials and recovery policy replace the development key.
IMAGE_FEATURES += "splash ssh-server-dropbear allow-root-login"
IMAGE_LINGUAS = " "
IMAGE_ROOTFS_EXTRA_SPACE = "524288"

inherit core-image extrausers

# iwd already confines control access to root, wheel, and netdev on the system
# bus. The kiosk joins only netdev; it receives no general administrative role.
EXTRA_USERS_PARAMS = "groupadd netdev; usermod -a -G netdev weston;"

SYSTEMD_DEFAULT_TARGET = "graphical.target"
