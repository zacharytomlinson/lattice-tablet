FILESEXTRAPATHS:prepend := "${THISDIR}/files:${THISDIR}/../../../../assets:"

SRC_URI:append = " \
    file://latticeos-boot.png \
    file://latticeos.plymouth \
    file://latticeos.script \
    file://plymouthd.conf \
    "

# The tablet boots directly from its root filesystem, so it needs neither
# dracut nor Plymouth's initrd payload. Build only the DRM/udev/systemd script
# surface required for one static LatticeOS handoff.
PLYMOUTH_THEMES = "script"
PACKAGECONFIG = "drm script systemd udev"
PLYMOUTH_BACKGROUND_COLOR = "0x070b0d"
PLYMOUTH_BACKGROUND_START_COLOR_STOP = "0x070b0d"
PLYMOUTH_BACKGROUND_END_COLOR_STOP = "0x070b0d"

do_install:append() {
    install -d ${D}${datadir}/plymouth/themes/latticeos
    install -m0644 ${WORKDIR}/latticeos-boot.png \
        ${D}${datadir}/plymouth/themes/latticeos/latticeos-boot.png
    install -m0644 ${WORKDIR}/latticeos.plymouth \
        ${D}${datadir}/plymouth/themes/latticeos/latticeos.plymouth
    install -m0644 ${WORKDIR}/latticeos.script \
        ${D}${datadir}/plymouth/themes/latticeos/latticeos.script
    install -Dm0644 ${WORKDIR}/plymouthd.conf \
        ${D}${sysconfdir}/plymouth/plymouthd.conf
}

FILES:${PN}:append = " \
    ${datadir}/plymouth/themes/latticeos \
    ${sysconfdir}/plymouth/plymouthd.conf \
    "
