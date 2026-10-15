FILESEXTRAPATHS:prepend := "${THISDIR}/files:"

SRC_URI:append = " file://main.conf"

do_install:append() {
    install -Dm0644 ${WORKDIR}/main.conf ${D}${sysconfdir}/iwd/main.conf
}

FILES:${PN}:append = " ${sysconfdir}/iwd/main.conf"
