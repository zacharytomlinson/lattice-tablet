# Linux peer client

This is the first tablet backend component. It communicates with the maintained
XIAO application in `../firmware/nrf52840` using `lattice-host-link`, the same small
binary codec used by the kernel. The nRF and Linux host are one network participant.
The MCU owns the radio, formation, maintained neighbors, routing, and read identity.
The Linux side decides which references to follow. No server inventory is required.

```sh
cargo +stable build --manifest-path peer/Cargo.toml --locked
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE status
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE links
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE links OWNER_HEX
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE links OWNER_HEX VIA_HEX
```

Choose the actual `Lattice_Tablet_radio` device. `links` without an owner reads the
attached resident's maintained links. For a discovered remote neighbor, provide
its printed address. To follow a reference beyond direct range, provide the owner
and the address of the vertex that advertised it as `VIA_HEX`. Enumeration reads
one record per cursor and stops at its explicit end. It fails if membership changes
during the enumeration, rather than presenting mixed revisions as one snapshot.
An empty list is valid while neighboring peers establish their reciprocal links.

The serial client selects raw 115200-baud settings and DTR, takes exclusive access,
and correlates every response. This baud value configures the CDC interface; it
does not rate-limit USB to a physical UART. All link data is binary. The CLI formats
human-readable output only after decoding. Device failures and timeouts are
reported; requests are not automatically retried. Closing the client lowers DTR
and cancels its pending host exchange without withdrawing the nRF from the network.

The `Client` library is independent of Slint and the OS image. Status and `ReadLinks`
are implemented; surface/state reads, transformer invocation, delta delivery,
continuous daemon operation, and UI integration remain to be added. The image
builder packages the CLI but does not start a peer service yet.
