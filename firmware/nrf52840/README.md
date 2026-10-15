# Tablet-attached XIAO nRF52840

Maintained application for the XIAO nRF52840/Sense used as the tablet's radio.
It starts the ordinary Lattice kernel with an empty local appliance surface and
one low-priority USB task. The kernel selects its network and maintains/forwards
links even while USB is closed. Its optional local host endpoint services Linux
requests under the same radio identity. No diagnostic traffic or text console is
included in this image.

The USB task uses Embassy's peripheral driver and CDC stack, polled once per kernel
millisecond through the existing RTOS. No Embassy executor or heap is installed.
Current binary records fit in one endpoint packet; the decoder treats USB as a
stream and handles split/combined records. The development identity is `c0de:cafd`,
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

The Linux client is in `../../peer`. The first hardware acceptance used a second
nRF running the kernel's ordinary port fixture, without the radio probe or peer
observer. The host enumerated the attached peer, read the other peer over radio,
and received the reciprocal relationship and explicit end-of-list. A nonexistent
owner returned a timeout and the next valid read completed. One attempt returned
a transmit error; a fresh read succeeded. Reopening USB and software-controlled
application reflash also worked. This validates that path;
it is not a range, loss-rate, or multi-hop hardware measurement.
