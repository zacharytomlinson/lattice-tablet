# Independent StarLite installation

This image owns its Linux build, root filesystem, application, services, and state.
It has no runtime mode that converts an existing installation. The first image uses
a removable drive; an internal installation is a later, explicitly targeted action.

| Identity | Value |
| --- | --- |
| Yocto distro / hostname | `lattice-tablet` |
| Layer / image | `meta-lattice-tablet` / `lattice-tablet-image` |
| Build cache | `~/.cache/lattice-yocto` |
| Kernel release suffix | `-lattice-standard` |
| EFI title / filesystem label | `Lattice Tablet` / `LATTICEBOOT` |
| Removable root / persistent state labels | `lattice-root` / `lattice-state` |
| Kiosk / binary | `lattice-tablet.service` / `/usr/bin/lattice-tablet` |
| Application data / config | `/var/lib/lattice-tablet` / `/var/lib/lattice-tablet/config` |

Persistent `/var/lib` belongs to this installation, including local iwd settings.
Do not share its state partition or copy another installation's identity into it.
The Weston rotation/backlight plugin and power defaults are platform support;
they do not contain Lattice peer behavior.

## Build

Use an x86-64 Linux host supported by Yocto Scarthgap, a stable Rust toolchain
capable of building the locked Slint dependencies, and the standard Yocto host
packages (including gawk, diffstat, chrpath, gcc/g++, make, git, Python 3, cpio,
unzip, xz, lz4 and zstd). Reserve ample space for the full Linux image build. The
pinned sources are recorded in `yocto/sources.conf`.

```sh
yocto/scripts/prepare-sources.sh
yocto/scripts/build-starlite-image.sh
```

`LATTICE_YOCTO_WORKDIR` changes the build directory. Use an independent directory,
not an existing installation's build tree. Preparation refuses to reset a dirty
or differently pinned source checkout. The image builder compiles the Rust
frontend, packages it with the Linux image, and prints the WIC artifact paths.

`build-starlite-image.sh --prepare-only` builds/packages the frontend and writes
the BitBake configuration without running the image build. That is not a bootable
image. The recipe packages a host-built GNU/Linux executable, as the source
platform did; the resulting image's dynamic libraries must satisfy that artifact.
A successful host build does not establish target ABI or device acceptance.

Set `LATTICE_SSH_AUTHORIZED_KEYS` to an explicitly selected public authorized-keys
file if development SSH access is needed. Otherwise the image receives an empty
key file. No existing user's keys, Wi-Fi credentials, or remote pairing state are
silently imported. The bring-up image retains key-only root SSH; it is not a
finished production servicing policy.

The current source checks cover the frontend, portable rotation logic, and image
configuration. A full image build and StarLite boot/display/touch/rotation/power
acceptance remain required before this installation can replace a working one.

## Installation boundary

The removable WIC layout has an EFI partition, one root, and separate state.
`install-starlite-internal.sh` is retained for a later internal installation. It
requires explicit pre-existing, unmounted, empty destination partitions with
Lattice labels; it does not create or resize them. Its optional second root does
not implement an automatic A/B update mechanism. See its `--help` before any use.

Keep boot/root/state storage separate from the working installation. Destination
selection, partitioning, flashing, and boot validation are separate from source
migration and have not been performed by establishing this repository.
