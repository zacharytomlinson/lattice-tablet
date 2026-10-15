# Lattice tablet

Independent StarLite Linux OS image and Slint frontend for a tablet participating
in a Lattice network. The initial installation is vertices plus the tablet; a
nucleus is optional. The tablet needs an attached 802.15.4 radio, such as an nRF52840.

This foundation carries the display, touch, rotation, backlight, power support,
visual components, local Wi-Fi settings, and kiosk supervision forward into its
own installation. It starts with an empty Home screen. Lattice discovery, surface
reading, transformer invocation, state deltas, and the attached-radio transport
are the next increment; no peer backend or appliance control is connected yet.
Camera layouts remain available, but image delivery is not implemented.

The [architecture map](docs/ARCHITECTURE.md) distinguishes implemented support
from planned peer behavior. [UPSTREAM.md](UPSTREAM.md) records the extraction and
retained licensing. No adjacent application checkout or consumer SDK is required.

## Frontend

Use a current stable Rust toolchain. The default build targets Linux Wayland with
the GPU renderer; it requires a working Wayland compositor and graphics stack.

```sh
cargo build --locked
cargo run --locked
```

The application is a fullscreen kiosk. Starting it does not scan or change Wi-Fi;
Settings starts the local iwd worker only after an explicit action. On hosts without
iwd the application still runs, but those settings cannot manage Wi-Fi.

```sh
cargo test --locked --no-default-features --features software
```

This runs the local settings/readiness checks and headless rendering of the empty
Home and Settings at landscape and portrait sizes. Previews are written to
`target/previews/`. No radio or Wi-Fi scans occur in these checks.
[UI ownership and styling](ui/README.md) describe the retained component vocabulary.

## OS image

The [image instructions](docs/OS_IMAGE.md) cover pinned source preparation,
application packaging, and building the independent Yocto image. Linux/Yocto
sources are fetched during preparation; they are not vendored into this repository.

This repository does not install alongside another application's root or state
filesystem. Boot selection chooses a complete OS installation. Building an image
never flashes the tablet or changes its current installation.
