# Lattice tablet

Read README.md and docs/ARCHITECTURE.md before implementation. This repository
owns an independent tablet OS image and application. It is not a second mode of
another installation. Keep its boot, root, state, service, configuration, and build
identities separate. Never deploy to or repartition a device as part of a source
migration. Actual installation needs explicitly selected destination storage.

For UI changes, read ui/README.md. Reuse Theme and shared Slint components,
keep identity separate from activation, and review landscape before portrait.
The interface displays observed state; command acceptance is not an observation.
No fake household data or silent mock backend belongs in ordinary startup.

Keep the Linux/Yocto platform support, Lattice peer/backend behavior, and Slint
presentation separate. Do not recreate an authoritative whole-home server
projection as a compatibility layer. Do not import the previous consumer SDK.
Userspace services may move into kernel facilities later; keep OS-specific I/O
at its owning boundary and evolve that boundary when concrete work needs it.

One-off radio/localization experiments stay outside the repository. Keep useful
portable checks with the code they exercise. Preserve upstream and asset licenses.
Update the architecture map when ownership changes. No release, deployment, or
hardware-validation claim follows from a host build alone.
