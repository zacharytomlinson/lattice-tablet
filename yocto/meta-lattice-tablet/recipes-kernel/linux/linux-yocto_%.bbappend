FILESEXTRAPATHS:prepend := "${THISDIR}/files:"

SRC_URI += " \
    file://starlite-input.cfg \
    file://0001-i2c-hid-acpi-bind-starlite-goodix-id.patch \
"

# Independent kernel/module release identity for this installation.
LINUX_VERSION_EXTENSION = "-lattice-standard"
