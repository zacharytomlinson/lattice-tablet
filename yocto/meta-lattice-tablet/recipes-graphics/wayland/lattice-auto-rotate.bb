SUMMARY = "StarLite accelerometer-driven Weston output rotation"
LICENSE = "MIT"
LIC_FILES_CHKSUM = "file://${COMMON_LICENSE_DIR}/MIT;md5=0835ade698e0bcf8506ecda2f7b4f302"

SRC_URI = "file://lattice-auto-rotate.c file://lattice-backlight.c file://lattice-backlight.h file://lattice-motion.h file://lattice-motion-test.c"

S = "${WORKDIR}"

inherit pkgconfig

DEPENDS = "weston systemd"

do_compile() {
    ${BUILD_CC} ${BUILD_CFLAGS} -Wall -Wextra -Werror \
        ${WORKDIR}/lattice-motion-test.c -o lattice-motion-test ${BUILD_LDFLAGS} -lm
    ./lattice-motion-test
    ${CC} ${CFLAGS} ${CPPFLAGS} -fPIC -shared \
        $(pkg-config --cflags weston libsystemd) \
        ${WORKDIR}/lattice-auto-rotate.c \
        ${WORKDIR}/lattice-backlight.c \
        -o lattice-auto-rotate.so \
        ${LDFLAGS} $(pkg-config --libs weston libsystemd) -lm
}

do_install() {
    install -Dm0755 ${B}/lattice-auto-rotate.so \
        ${D}${libdir}/weston/lattice-auto-rotate.so
}

FILES:${PN} = "${libdir}/weston/lattice-auto-rotate.so"
