FILESEXTRAPATHS:prepend := "${THISDIR}/files:"

SRC_URI:append = " \
    file://weston-recovery.conf \
    file://lattice-drm-ready.service \
    "

do_install:append() {
    install -Dm0644 ${WORKDIR}/weston-recovery.conf \
        ${D}${systemd_system_unitdir}/weston.service.d/10-lattice-recovery.conf
    install -Dm0644 ${WORKDIR}/lattice-drm-ready.service \
        ${D}${systemd_system_unitdir}/lattice-drm-ready.service
}

FILES:${PN} += " \
    ${systemd_system_unitdir}/weston.service.d/10-lattice-recovery.conf \
    ${systemd_system_unitdir}/lattice-drm-ready.service \
    "

SYSTEMD_SERVICE:${PN}:append = " lattice-drm-ready.service"
