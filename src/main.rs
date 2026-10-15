use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Model, ModelRc, Timer, TimerMode, VecModel};

mod network_settings;
#[cfg(target_os = "linux")]
mod systemd_health;
#[cfg(all(test, feature = "software"))]
mod ui_tests;

slint::include_modules!();

fn initialize_models(window: &MainWindow) {
    window.set_rooms(ModelRc::new(VecModel::<RoomItem>::default()));
    window.set_lights(ModelRc::new(VecModel::<LightItem>::default()));
    window.set_climates(ModelRc::new(VecModel::<ClimateItem>::default()));
    window.set_media(ModelRc::new(VecModel::<MediaItem>::default()));
    window.set_media_apps(ModelRc::new(VecModel::<MediaAppItem>::default()));
    window.set_shades(ModelRc::new(VecModel::<ShadeItem>::default()));
    window.set_cameras(ModelRc::new(VecModel::<CameraItem>::default()));
    window.set_generic_objects(ModelRc::new(VecModel::<GenericObjectItem>::default()));
    window.set_networks(ModelRc::new(VecModel::<NetworkItem>::default()));
}

fn bind_navigation(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_open_room(move |id| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        if let Some(room) = window.get_rooms().iter().find(|room| room.id == id) {
            window.set_room_name(room.name);
            window.set_object_count(room.device_count);
            window.set_current_view("room".into());
        }
    });
}

fn update_networks(model: &ModelRc<NetworkItem>, rows: Vec<NetworkItem>) {
    let retained = model
        .as_any()
        .downcast_ref::<VecModel<NetworkItem>>()
        .expect("network model is initialized once");
    for (index, row) in rows.iter().enumerate() {
        if retained
            .row_data(index)
            .is_some_and(|current| current.id == row.id)
        {
            if retained.row_data(index).as_ref() != Some(row) {
                retained.set_row_data(index, row.clone());
            }
            continue;
        }
        if let Some(existing) = (index..retained.row_count()).find(|&existing| {
            retained
                .row_data(existing)
                .is_some_and(|current| current.id == row.id)
        }) {
            retained.remove(existing);
        }
        retained.insert(index, row.clone());
    }
    while retained.row_count() > rows.len() {
        retained.remove(retained.row_count() - 1);
    }
}

/// Local Wi-Fi management is created only by an explicit user action. Starting
/// this shell or rendering its settings never scans/reconfigures the host adapter.
fn bind_network_settings(window: &MainWindow) -> Timer {
    use network_settings::NetworkSettings;
    let settings = Rc::new(RefCell::new(None::<NetworkSettings>));
    let control = Rc::clone(&settings);
    window.on_refresh_networks(move || {
        control
            .borrow_mut()
            .get_or_insert_with(NetworkSettings::start)
            .scan();
    });
    let control = Rc::clone(&settings);
    let weak = window.as_weak();
    window.on_connect_network(move |id, password| {
        control
            .borrow_mut()
            .get_or_insert_with(NetworkSettings::start)
            .connect(id.to_string(), password.to_string());
        if let Some(window) = weak.upgrade() {
            window.set_network_password("".into());
        }
    });
    let control = Rc::clone(&settings);
    window.on_disconnect_network(move || {
        if let Some(settings) = control.borrow().as_ref() {
            settings.disconnect();
        }
    });
    let weak = window.as_weak();
    window.on_keyboard_key(move |key| {
        if let Some(window) = weak.upgrade() {
            window.set_network_password(format!("{}{key}", window.get_network_password()).into());
        }
    });
    let weak = window.as_weak();
    window.on_keyboard_backspace(move || {
        if let Some(window) = weak.upgrade() {
            let mut value = window.get_network_password().to_string();
            value.pop();
            window.set_network_password(value.into());
        }
    });
    let weak = window.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let Some(window) = weak.upgrade() else {
            return;
        };
        let control = settings.borrow();
        let Some(settings) = control.as_ref() else {
            return;
        };
        let snapshot = settings.snapshot();
        let rows: Vec<_> = snapshot
            .networks
            .iter()
            .map(|network| NetworkItem {
                id: network.id.clone().into(),
                name: network.name.clone().into(),
                security: network.security.clone().into(),
                signal: network.signal,
                connected: network.connected,
            })
            .collect();
        update_networks(&window.get_networks(), rows);
        window.set_network_available(snapshot.available);
        window.set_network_scanning(snapshot.scanning);
        window.set_network_status(snapshot.status.into());
    });
    timer
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "wayland")]
    {
        use i_slint_backend_winit::winit;
        let backend = i_slint_backend_winit::Backend::builder()
            .with_window_attributes_hook(|attributes| {
                use winit::platform::wayland::WindowAttributesExtWayland;
                use winit::window::Fullscreen;
                attributes
                    .with_decorations(false)
                    .with_fullscreen(Some(Fullscreen::Borderless(None)))
                    .with_name("io.palodurosystems.LatticeTablet", "lattice-tablet")
            })
            .build()?;
        slint::platform::set_platform(Box::new(backend))?;
    }
    let window = MainWindow::new()?;
    initialize_models(&window);
    bind_navigation(&window);
    let _network_timer = bind_network_settings(&window);
    #[cfg(target_os = "linux")]
    let _systemd_watchdog = systemd_health::install(&window)?;
    window.run()?;
    Ok(())
}
