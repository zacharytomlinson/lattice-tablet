# Tablet UI

Slint components are compiled into the frontend. Future Lattice backend code
supplies observed values and available operations through tablet-owned presentation
records. Protocol packets, IR expressions, and MCU layouts do not become UI types.

| Responsibility | Owner |
| --- | --- |
| Kiosk shell, navigation, callback forwarding | `app.slint` |
| Presentation values and opaque interaction handles | `models.slint` |
| Palette, backdrop, identity, activation and navigation | `foundation.slint` |
| Shared touch/input mechanics | `common_controls.slint` |
| Generic controls and appliance compositions | `generic_*.slint`, `lighting.slint`, `climate.slint`, `media.slint`, `shade.slint`, `camera.slint` |
| Home and room layout | `home_screen.slint`, `room_screen.slint` |
| Explicit local Wi-Fi interaction | `settings.slint`, `src/network_settings.rs` |

Reuse Theme tokens and shared controls. Appliance identity stays neutral; the
owning control shows activation. Power/actions use shared touch targets and
interaction styling. Review landscape first, then portrait. Keep the stationary
AtmosphereBackdrop behind transparent screens; its textures and generation are
documented in [assets/backgrounds](../assets/backgrounds/README.md).

Rust installs each top-level VecModel once. Refreshes update retained rows rather
than replacing whole models, so touch gestures can continue while observations
arrive. Slint owns transient gesture/preview state. Command acceptance must never
be rendered as an observed appliance change.

The foundation has no household fixtures or mock backend in normal startup.
It deliberately leaves models empty until the peer backend can populate them.
Retained appliance callbacks are integration points, not implemented controls.
Connection details belong in Settings; primary screens stay focused on appliances.
The Lattice radio panel currently reports no connection because transport is not
implemented. Camera layouts show an unavailable preview, never a fixture photo.
