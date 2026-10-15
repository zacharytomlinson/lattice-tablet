use super::{MainWindow, NetworkItem, bind_navigation, initialize_models, update_networks};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, TargetPixel,
};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, Model, PhysicalSize};
use std::{fs::File, io::BufWriter, rc::Rc, time::Instant};

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    static START: Instant = Instant::now();
}
struct SoftwarePlatform;
impl Platform for SoftwarePlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(Rc::clone))
    }
    fn duration_since_start(&self) -> std::time::Duration {
        START.with(Instant::elapsed)
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
struct RgbPixel {
    red: u8,
    green: u8,
    blue: u8,
}

impl TargetPixel for RgbPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let inverse_alpha = 255_u32 - u32::from(color.alpha);
        self.red =
            (u32::from(color.red) + u32::from(self.red) * inverse_alpha / 255).min(255) as u8;
        self.green =
            (u32::from(color.green) + u32::from(self.green) * inverse_alpha / 255).min(255) as u8;
        self.blue =
            (u32::from(color.blue) + u32::from(self.blue) * inverse_alpha / 255).min(255) as u8;
    }

    fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

fn render(window: &Rc<MinimalSoftwareWindow>, width: u32, height: u32) -> Vec<RgbPixel> {
    window.set_size(PhysicalSize::new(width, height));
    window.request_redraw();
    let mut pixels = vec![RgbPixel::default(); (width * height) as usize];
    assert!(window.draw_if_needed(|renderer| {
        renderer.render(pixels.as_mut_slice(), width as usize);
    }));
    pixels
}

fn save_png(path: &str, pixels: &[RgbPixel], width: u32, height: u32) {
    let mut encoder = png::Encoder::new(
        BufWriter::new(File::create(path).expect("preview file is writable")),
        width,
        height,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("PNG header is valid");
    let bytes = pixels
        .iter()
        .flat_map(|pixel| [pixel.red, pixel.green, pixel.blue])
        .collect::<Vec<_>>();
    writer
        .write_image_data(&bytes)
        .expect("preview pixels are valid");
}

#[test]
fn empty_installation_and_settings_render_without_a_backend() {
    slint::platform::set_platform(Box::new(SoftwarePlatform)).unwrap();
    let surface = WINDOW.with(Rc::clone);
    surface.set_size(PhysicalSize::new(1180, 720));
    let ui = MainWindow::new().unwrap();
    initialize_models(&ui);
    bind_navigation(&ui);
    assert_eq!(ui.get_rooms().row_count(), 0);
    assert_eq!(ui.get_lights().row_count(), 0);
    assert!(!ui.get_state_known());
    ui.invoke_open_room("absent".into());
    assert_eq!(ui.get_current_view().as_str(), "home");
    ui.show().unwrap();
    std::fs::create_dir_all("target/previews").unwrap();
    for (name, view, width, height) in [
        ("home-landscape", "home", 1180, 720),
        ("home-portrait", "home", 720, 1180),
        ("settings-landscape", "settings", 1180, 720),
        ("settings-portrait", "settings", 720, 1180),
    ] {
        ui.set_current_view(view.into());
        let pixels = render(&surface, width, height);
        assert!(pixels.iter().any(|pixel| *pixel != pixels[0]));
        save_png(
            &format!("target/previews/{name}.png"),
            &pixels,
            width,
            height,
        );
    }
    // A refresh retains the model and the unaffected row's identity as a
    // stronger signal, insertion, reorder and removal arrive.
    let model = ui.get_networks();
    let first = NetworkItem {
        id: "first".into(),
        name: "First".into(),
        ..Default::default()
    };
    let second = NetworkItem {
        id: "second".into(),
        name: "Second".into(),
        ..Default::default()
    };
    let stronger = NetworkItem {
        signal: 90,
        ..first.clone()
    };
    for rows in [
        vec![first.clone()],
        vec![second.clone(), stronger.clone()],
        vec![stronger, second],
        vec![],
    ] {
        update_networks(&model, rows.clone());
        assert!(model == ui.get_networks());
        assert_eq!(model.iter().collect::<Vec<_>>(), rows);
    }
    ui.hide().unwrap();
}
