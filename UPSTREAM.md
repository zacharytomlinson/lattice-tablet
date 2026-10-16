# Source provenance

The initial platform and frontend sources were selectively extracted from
https://github.com/zacharytomlinson/jarvis-tablet at commit
`de56bf3` on 2026-10-15. That repository and its local documentation edits were
left untouched. This repository does not depend on that checkout or its SDK.

Retained: Slint visual primitives and appliance compositions; StarLite Linux
configuration and touchscreen patch; Weston rotation/backlight plugin and its
host checks; power defaults; systemd readiness/watchdog integration; iwd settings.

Excluded: Home Consumer SDK archives, server pairing/projection/retry machinery,
Matter commissioning, server-backed automation authoring, household fixtures,
localization experiments, deployed identity/configuration, and build artifacts.

Font and icon license texts remain beside their assets. Linux patch attribution
and recipe license declarations are retained. The proprietary application is not
being relicensed by this extraction.

The tablet radio application uses `lattice-vertex`. Its XIAO memory layout and
application-only packaging helper come from that kernel's maintained Nordic port
support. The Linux client and kernel consume one `lattice-peer-wire` codec. Both
Cargo manifests pin the kernel repository revision instead of relying on local
sibling checkouts. Radio experiments and reference images are not dependencies.
