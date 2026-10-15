SUMMARY = "Lattice tablet kiosk"
LICENSE = "CLOSED"

LATTICE_TABLET_ARTIFACT_DIR ?= "${THISDIR}/files"
FILESEXTRAPATHS:prepend := "${LATTICE_TABLET_ARTIFACT_DIR}:${THISDIR}/files:"

SRC_URI = "\
    file://lattice-tablet \
    file://lattice-peer \
    file://lattice-tablet.service \
    file://journald-bringup.conf \
    file://authorized_keys \
    "

S = "${WORKDIR}"

inherit systemd

SYSTEMD_SERVICE:${PN} = "lattice-tablet.service"
SYSTEMD_AUTO_ENABLE:${PN} = "enable"

do_install() {
    install -Dm0755 ${WORKDIR}/lattice-tablet ${D}${bindir}/lattice-tablet
    install -Dm0755 ${WORKDIR}/lattice-peer ${D}${bindir}/lattice-peer
    install -Dm0644 ${WORKDIR}/lattice-tablet.service \
        ${D}${systemd_system_unitdir}/lattice-tablet.service
    install -Dm0644 ${WORKDIR}/journald-bringup.conf \
        ${D}${sysconfdir}/systemd/journald.conf.d/10-lattice-bringup.conf

    install -d -m0700 ${D}${ROOT_HOME}/.ssh
    install -m0600 ${WORKDIR}/authorized_keys ${D}${ROOT_HOME}/.ssh/authorized_keys

    # The pinned GNU/Linux artifact uses the conventional x86-64 interpreter
    # path. Scarthgap's merged-/usr rootfs keeps the same loader in ${libdir}.
    install -d ${D}/lib64
    ln -s ../usr/lib/ld-linux-x86-64.so.2 ${D}/lib64/ld-linux-x86-64.so.2
}

FILES:${PN} += "\
    ${ROOT_HOME}/.ssh/authorized_keys \
    /lib64/ld-linux-x86-64.so.2 \
    ${sysconfdir}/systemd/journald.conf.d/10-lattice-bringup.conf \
    "
RDEPENDS:${PN} = "weston-init"

INSANE_SKIP:${PN} += "already-stripped"
