SUMMARY = "Measured StarLite power defaults"
LICENSE = "MIT"
LIC_FILES_CHKSUM = "file://${COMMON_LICENSE_DIR}/MIT;md5=0835ade698e0bcf8506ecda2f7b4f302"

SRC_URI = "file://lattice-power.conf file://audio-powersave.conf"
S = "${WORKDIR}"

inherit allarch

do_install() {
    install -Dm0644 ${WORKDIR}/lattice-power.conf \
        ${D}${nonarch_libdir}/tmpfiles.d/lattice-power.conf
    install -Dm0644 ${WORKDIR}/audio-powersave.conf \
        ${D}${sysconfdir}/modprobe.d/lattice-audio-powersave.conf

    # Older StarLite boot entries still request the nonexistent PC serial port.
    install -d ${D}${systemd_system_unitdir}
    ln -s /dev/null ${D}${systemd_system_unitdir}/serial-getty@ttyS0.service
}

FILES:${PN} += "${nonarch_libdir}/tmpfiles.d/lattice-power.conf ${systemd_system_unitdir}/serial-getty@ttyS0.service"
