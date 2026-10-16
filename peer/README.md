# Linux peer client

This tablet backend communicates with the XIAO application in
`../firmware/nrf52840`. Host and MCU share one resident network identity. The MCU
owns radio operation, formation, maintained neighbors, and routing; Linux chooses
which references and appliance surfaces to consume. No server inventory is required.
Both sides use the shared `lattice-peer-wire` host codec and `lattice-ir` values.

```sh
cargo +stable build --manifest-path peer/Cargo.toml --locked
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE watch [COUNT]
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE status
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE links
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE links OWNER_HEX [VIA_HEX]
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE surface OWNER_HEX [OUTPUT_FILE]
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE state OWNER_HEX ATTRIBUTE_HEX
peer/target/debug/lattice-peer /dev/serial/by-id/DEVICE invoke OWNER_HEX TRANSFORMER_HEX u8:75
```

Choose the actual `Lattice_Tablet_radio` device. Owner and remote-reference arguments
are hexadecimal; typed values use `bool:true`, `u8:42`, `i32:-1`, etc. All eight
integer widths/signedness combinations are supported. Unknown handles are never
local pointers. Use references from the current surface, not from an old firmware.

`links` without an owner enumerates the attached resident. To follow an advertised
reference beyond direct range, pass the new owner and advertising vertex as `VIA`.
Enumeration stops at the explicit end and rejects a membership revision change.
An empty list is valid while adjacent peers establish their reciprocal links.

`surface` requests one immutable 67-byte range at a time. Missing, inconsistent, or
incomplete ranges fail the operation; no partial file is saved. `surface.rs`
structurally walks the complete LSF body without evaluating IR and returns a
`SurfaceSummary` of declared types/handles and axiom/invariant counts. The optional
output file retains the **entire** binary mathematics, including predicates and
effects. This is not a full owned IR decoder or a semantic verifier. Definitions
are immutable for a firmware lifetime; this protocol has no content digest to
detect a simultaneous reflash with an equal-length description.

`state` returns an established value and change watermark, or an explicit read
failure. A successful read may report that its value is unavailable. `invoke`
submits one typed native call. `Invoked` means the function returned success;
**registered changes or explicit state reads establish what changed**. Native
functions enforce device limits; no IR interpreter is installed in the MCU. Arguments must fit one RF frame
(61 encoded bytes, at most 30 scalars); calls are not fragmented.

The serial client selects raw 115200-baud settings, DTR, and exclusive access.
The baud setting does not rate-limit CDC to a physical UART. Host v3 records are
binary and may span USB endpoint packets; the CLI formats text only after decoding.
It seeds its 32-bit token sequence from OS entropy per connection. The MCU uses
the first host read token to seed its separate 16-bit radio-read counter, which then
advances across kinds and USB sessions. This avoids deliberately restarting at
read 1 after reboot while a remote retains that identity. Both namespaces remain
probabilistic within bounded/volatile history.

Requests are never automatically retried. Timeout, USB loss, or a transmit error
leaves an invocation's outcome unconfirmed; reopening and invoking again is a new
call, not recovery of the previous one. Read established state before deciding what
to do. Closing USB cancels host completions without withdrawing the resident nRF
or undoing an already-reached operation.

`watch` opens a persistent connection and waits for unsolicited registered changes.
It sends no status, subscription, or state-read packet. `Client::wait_event(None)`
blocks in Linux `poll(2)` until USB data is readable; this system call sends no
Lattice polling request. A finite timeout is also available without closing the
session. A consumer can invoke a transformer and wait for changes on the same
`Client`; events interleaved with command replies are retained, including those
coalesced after a reply in one USB read. Command completion remains distinct.

The companion holds eight pending changes; Linux retains 32 while its caller is
handling other operations. Both evict the oldest change when full. `Event::Lagged`
reports the count and distinguishes companion overflow from client overflow.
Loss notices are delivered before retained changes; they are gap notifications,
not positions in a complete history or counts of missed RF frames. Opening a new
connection starts a fresh stream with no snapshot/history replay. An idle network
produces no state events. `watch` does not invent one to demonstrate liveness.

Status, links, surfaces, state reads, native invocation, and pushed changes are
implemented. Continuous daemon behavior and UI integration remain next. The image
includes this CLI but does not start a peer service. The library and CLI remain independent of Slint and of the currently installed tablet OS.

The client has also run on the physical StarLite through its USB-attached nRF:
link enumeration, a complete remote surface, and native invocation followed by a
pushed registered change. The current tablet kernel needed a matching CDC ACM
module loaded at runtime. This was a temporary `/run` deployment on the existing
installation; the independent Lattice image and UI have not been deployed.
See [the validation boundary](../docs/ARCHITECTURE.md#starlite-coprocessor-proof).
