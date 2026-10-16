# Tablet-attached XIAO nRF52840

Maintained application for the XIAO nRF52840/Sense used as the tablet's radio.
It starts the ordinary Lattice kernel with an empty local appliance surface and
one low-priority USB task. The kernel selects its network and maintains/forwards
links even while USB is closed. Its optional local host endpoint services Linux
requests and pushes registered changes under the same radio identity. No diagnostic
traffic or text console is included in this image.

The USB task uses Embassy's peripheral driver and CDC stack, polled once per kernel
millisecond through the existing RTOS. No Embassy executor or heap is installed.
Host v3 uses a fixed 128-byte record buffer plus a leading delimiter; current
records are at most 87 bytes and can span two endpoint packets. The decoder handles split/combined records, and writes
end with a short packet or ZLP. Control/DFU handling stays active during those
writes, including when Linux is not draining IN. The USB loop selects incoming
requests or pending kernel records; it does not wait for a request to send an event.
DTR attaches/detaches the change stream. The MCU still services USB once per
millisecond, but Linux never sends periodic requests to obtain those changes.
The Nordic driver finishes DMA within a poll; cancelling a pending OUT wait to
send an event leaves the endpoint data intact. Each corresponding radio message
still fits one RF frame. The development identity is `c0de:cafd`,
product `Tablet radio`, with the chip's factory identifier as the serial number.
This VID/PID is for development, not a production USB allocation.

Build from this directory:

```sh
cargo +nightly build -Z build-std=core --release --locked
uv venv target/dfu-tools
uv pip install --python target/dfu-tools/bin/python \
  adafruit-nrfutil==0.5.3.post16 'setuptools<81'
python3 package_xiao.py target/thumbv7em-none-eabihf/release/lattice-tablet-radio \
  target/tablet-radio-xiao.zip --nrfutil target/dfu-tools/bin/adafruit-nrfutil
```

The linker keeps the XIAO boot chain: application flash is `0x27000..0xED000`.
`package_xiao.py` verifies every application address and both initial vectors;
it never packages a bootloader or SoftDevice replacement. The RTOS does not enable
the installed SoftDevice. Software updates use the existing serial DFU bootloader:

```sh
target/dfu-tools/bin/adafruit-nrfutil --verbose dfu serial \
  --package target/tablet-radio-xiao.zip --port /dev/ttyACM_NUMBER \
  --baudrate 115200 --singlebank --touch 1200
```

Identify the actual board before uploading. The application enters serial DFU after
1200 baud with DTR low for 100 ms. Factory firmware also supports this request;
other firmware may have its own entry mechanism. A device already in Seeed serial
DFU (`2886:0045` on the Sense) needs no `--touch`. This uses USB only and never
changes the tablet's installed OS.

The Linux client is in `../../peer`. Link enumeration was first verified against
the ordinary Nordic port fixture. Surface/state/call validation uses its maintained
`test-appliance` feature: a software dimmer declaring two states, a native operation,
an axiom, and an invariant. The client reconstructed its 195-byte surface, obtained
handles from that description, set its level through the native operation, and read
back registered level/power changes. Wrong argument types and values beyond the
native range returned distinct failures. This validates kernel dispatch and fixture
state, not a physical dimmer output. Some radio reads timed out; no mutation was
automatically retried. Range/reliability investigation remains paused.

The earlier host v2 image passed surface -> invocation -> registered-state reads,
and a subsequent read with the remote console closed. Earlier closed-console and
post-reflash reads timed out, including a rerun with the console open; no single
cause was established for all failures. Read-token reuse across MCU restart was
fixed and covered by a regression check. Range/reliability work remains separate.
Temporary status images and register notes stay in the external lab.

Host v3 also delivered real power changes and a later level change (108) to the
Linux client over the same persistent connection used for native calls, with no
`ReadState` requests. An idle `watch` was traced blocking for USB input with no
serial writes. The final Git-pinned image also delivered level 110 at sequence 8,
and software DFU entry still worked. Some surface reads and a native call timed
out; no mutation was automatically retried. Two-change bursts were incomplete at
the host without an overflow notice. No complete-delivery or
root-cause claim follows; the RF investigation is tracked beside the kernel transport. Overflow and
reply/event interleaving are covered by portable checks beside their owners.


The current kernel pin includes receive turns between transmissions and bounded
completion of in-progress frames before cancellation. Forwarding remains enabled.
Controlled A/B/A testing identified the earlier repeated second-delta loss as a
half-duplex timing problem. Ordinary images then delivered 57/58 registered changes
in a 32-call run; 29 native results and 28/30 surface ranges arrived. Residual losses
remain under investigation. No retry was added to native calls or packet sends.
Diagnostic firmware and captures remain outside both maintained repositories.
