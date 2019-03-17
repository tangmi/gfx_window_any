use gfx_window_any::*;

struct TestApplication {
    window_targets: WindowTargets<Resources>,
}

impl Application<Resources> for TestApplication {
    fn new(
        factory: &mut impl gfx::Factory<Resources>,
        window: &winit::Window,
        window_targets: WindowTargets<Resources>,
    ) -> Self {
        // can get windows-specific stuff?
        // use winit::os::windows::WindowExt;
        // dbg!(window.get_hwnd());

        TestApplication { window_targets }
    }

    fn update(&mut self, frame_delta_in_seconds: f64) {
        let frame_delta_in_milli = frame_delta_in_seconds * 1_000_f64;
        // dbg!(frame_delta_in_milli);
    }

    fn render<C: gfx::CommandBuffer<Resources>>(
        &self,
        factory: &mut impl gfx::Factory<Resources>,
        encoder: &mut gfx::Encoder<Resources, C>,
    ) {
        // dbg!(&self.window_targets.color);
        encoder.clear(&self.window_targets.color, [1.0, 0.0, 0.0, 1.0]);
    }

    fn on_event(&mut self, event: winit::WindowEvent) {
        // dbg!(event);
    }
}

fn main() {
    env_logger::init();

    let window_builder = winit::WindowBuilder::new().with_title("hello");
    TestApplication::launch_new_window(window_builder);
}
