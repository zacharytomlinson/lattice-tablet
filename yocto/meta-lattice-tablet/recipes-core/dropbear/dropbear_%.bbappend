do_install:append() {
    # Bring-up access is public-key only. The image-level allow-root-login
    # feature removes Dropbear's default -w while this retains -s.
    sed -i 's/^DROPBEAR_EXTRA_ARGS=.*/DROPBEAR_EXTRA_ARGS="-s"/' \
        ${D}${sysconfdir}/default/dropbear
}
