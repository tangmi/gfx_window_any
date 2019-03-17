// #![deny(clippy::all)]
#![warn(clippy::pedantic)]
// #![deny(missing_docs)]

use gfx::Device;
use log::debug;
use log::error;
use log::info;
use log::warn;

#[cfg(not(target_os = "windows"))]
pub type ColorFormat = gfx::format::Rgba8;
#[cfg(target_os = "windows")]
pub type ColorFormat = gfx::format::Rgba8; // TODO i thinkg this is right

pub type DepthFormat = gfx::format::DepthStencil;

#[cfg(target_os = "windows")]
pub type Resources = gfx_device_dx11::Resources;

#[cfg(not(target_os = "windows"))]
pub type Resources = gfx_device_gl::Resources;

#[derive(Debug)]
pub struct WindowTargets<R: gfx::Resources> {
    pub color: gfx::handle::RenderTargetView<R, ColorFormat>,
    pub depth: gfx::handle::DepthStencilView<R, DepthFormat>,
    pub size: winit::dpi::LogicalSize,
    pub hidpi_factor: f64,
}

impl<R: gfx::Resources> WindowTargets<R> {
    pub fn aspect_ratio(&self) -> f32 {
        self.size.width as f32 / self.size.height as f32
    }

    pub fn physical_size(&self) -> winit::dpi::PhysicalSize {
        self.size.to_physical(self.hidpi_factor)
    }
}

pub trait Application<R>
where
    R: gfx::Resources,
{
    fn new(
        factory: &mut impl gfx::Factory<R>,
        window: &winit::Window,
        window_targets: WindowTargets<R>,
    ) -> Self;

    fn update(&mut self, frame_delta_in_seconds: f64);

    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_lossless)]
    fn update_2(&mut self, frame_delta: std::time::Duration) {
        // let frame_delta_in_seconds = frame_delta.as_float_secs() // nightly
        let frame_delta_in_seconds = (frame_delta.as_secs() as f64)
            + (frame_delta.subsec_nanos() as f64) / 1_000_000_000_f64;

        self.update(frame_delta_in_seconds);
    }

    fn render<C: gfx::CommandBuffer<R>>(
        &self,
        factory: &mut impl gfx::Factory<R>,
        encoder: &mut gfx::Encoder<R, C>,
    );

    fn on_event(&mut self, event: winit::WindowEvent);

    fn run(window_builder: winit::WindowBuilder)
    where
        Self: Sized + Application<Resources>,
    {
        run_loop::<Self, Resources, backend::Backend>(window_builder);
    }

    /// Called when the swapchain has changed size. Apps should recreate any screen-sized resources (e.g. G-buffers).
    fn on_swapchain_resized(
        &mut self,
        factory: &mut impl gfx::Factory<R>,
        window_targets: WindowTargets<R>,
    ) {
    }
}

#[allow(clippy::float_cmp)]
fn run_loop<A, R, B>(window_builder: winit::WindowBuilder)
where
    A: Sized + Application<R>,
    R: gfx::Resources,
    B: Backend<R>,
{
    use gfx::traits::Device;

    let mut events_loop = winit::EventsLoop::new();

    // TODO: handle window re-create?
    let BackendInit {
        mut window,
        mut device,
        mut factory,
        main_color,
        main_depth,
    } = B::init(window_builder, &events_loop);

    let (mut window_size, mut window_hidpi_factor) = {
        let window = B::get_winit_window(&window);

        (window.get_inner_size().unwrap(), window.get_hidpi_factor())
    };

    let window_targets = WindowTargets {
        color: main_color,
        depth: main_depth,
        size: window_size,
        hidpi_factor: window_hidpi_factor,
    };

    // pass `&mut window` into app to let them get os-specific stuff?
    let mut app = A::new(&mut factory, B::get_winit_window(&window), window_targets);
    let mut encoder: gfx::Encoder<R, B::CommandBuffer> = B::create_encoder(&mut factory);

    // For calculating the delta between frames for `Application::update`
    let mut frame_start_time = std::time::Instant::now();

    let mut running = true;
    while running {
        let mut new_size = None;
        let mut new_hidpi_factor = None;

        events_loop.poll_events(|event| {
            if let winit::Event::WindowEvent { event, .. } = event {
                match event {
                    winit::WindowEvent::CloseRequested => running = false,
                    winit::WindowEvent::Resized(size) => {
                        new_size = Some(size);
                    }
                    winit::WindowEvent::HiDpiFactorChanged(hidpi_factor) => {
                        new_hidpi_factor = Some(hidpi_factor);
                    }
                    _ => app.on_event(event),
                }
            }
        });

        let size_changed = new_size.map_or(false, |size| size != window_size);
        let hidpi_factor_changed = new_hidpi_factor.map_or(false, |hidpi_factor| {
            // #[allow(clippy::float_cmp)] // These values should be exactly the same (generally integral values, 0 < hidpi_factor < 5).
            hidpi_factor != window_hidpi_factor
        });
        if size_changed || hidpi_factor_changed {
            let winit_window = B::get_winit_window(&window);
            window_size = new_size.unwrap_or_else(|| winit_window.get_inner_size().unwrap());
            window_hidpi_factor =
                new_hidpi_factor.unwrap_or_else(|| winit_window.get_hidpi_factor());

            let new_window_targets = B::resize_swapchain(
                &mut window,
                &mut factory,
                &mut device,
                window_size,
                window_hidpi_factor,
            );

            if let Some(new_window_targets) = new_window_targets {
                app.on_swapchain_resized(&mut factory, new_window_targets);
            }

            // why?
            // continue;
        }

        app.update_2(frame_start_time.elapsed());
        frame_start_time = std::time::Instant::now();

        app.render(&mut factory, &mut encoder);
        B::flush(&mut encoder, &mut device);
        B::swap_buffers(&window);
        device.cleanup();
    }
}

/// Resources initialized from [`Backend::init`]
struct BackendInit<R: gfx::Resources, B: Backend<R> + ?Sized> {
    window: B::Window,
    device: B::Device,
    factory: B::Factory,
    main_color: gfx::handle::RenderTargetView<R, ColorFormat>,
    main_depth: gfx::handle::DepthStencilView<R, DepthFormat>,
}

trait Backend<R: gfx::Resources> {
    type Device: gfx::Device;
    type Factory: gfx::Factory<R>;
    type CommandBuffer: gfx::CommandBuffer<R>;
    type Window;

    fn init(
        window_builder: winit::WindowBuilder,
        events_loop: &winit::EventsLoop,
    ) -> BackendInit<R, Self>;

    fn create_encoder(factory: &mut Self::Factory) -> gfx::Encoder<R, Self::CommandBuffer>;

    /// NOTE: this is a method on [`Backend`] to avoid type resolution errors. The implementation should be the same for all backends.
    ///
    /// TODO: understand what is going on if I inline the implementation of `Backend::flush` into it's usage in `run_loop`.
    fn flush(encode: &mut gfx::Encoder<R, Self::CommandBuffer>, device: &mut Self::Device);

    fn get_winit_window(window: &Self::Window) -> &winit::Window;

    fn swap_buffers(window: &Self::Window);

    fn resize_swapchain(
        window: &mut Self::Window,
        factory: &mut Self::Factory,
        device: &mut Self::Device,
        new_size: winit::dpi::LogicalSize,
        new_hidpi_factor: f64,
    ) -> Option<WindowTargets<R>>;
}

#[cfg(target_os = "windows")]
mod backend {
    use super::*;
    use gfx::Factory;

    pub struct Backend {}

    impl super::Backend<Resources> for Backend {
        type Device = gfx_device_dx11::Deferred;
        type Factory = gfx_device_dx11::Factory;
        type CommandBuffer = gfx_device_dx11::CommandBuffer<gfx_device_dx11::DeferredContext>;
        type Window = gfx_window_dxgi::Window;

        fn init(
            window_builder: winit::WindowBuilder,
            events_loop: &winit::EventsLoop,
        ) -> BackendInit<Resources, Self> {
            let (window, device, mut factory, main_color) =
                gfx_window_dxgi::init::<ColorFormat>(window_builder, &events_loop).unwrap();

            let main_depth = factory
                .create_depth_stencil_view_only(window.size.0, window.size.1)
                .unwrap();

            let device = gfx_device_dx11::Deferred::from(device);

            // TODO shaders
            // device.get_shader_model()

            BackendInit {
                window,
                device,
                factory,
                main_color,
                main_depth,
            }
        }

        fn create_encoder(
            factory: &mut Self::Factory,
        ) -> gfx::Encoder<Resources, Self::CommandBuffer> {
            factory.create_command_buffer_native().into()
        }

        fn get_winit_window(window: &Self::Window) -> &winit::Window {
            &window.inner
        }

        fn flush(
            encode: &mut gfx::Encoder<Resources, Self::CommandBuffer>,
            device: &mut Self::Device,
        ) {
            encode.flush(device);
        }

        fn swap_buffers(window: &Self::Window) {
            window.swap_buffers(1);
        }

        fn resize_swapchain(
            window: &mut Self::Window,
            factory: &mut Self::Factory,
            device: &mut Self::Device,
            new_size: winit::dpi::LogicalSize,
            new_hidpi_factor: f64,
        ) -> Option<WindowTargets<Resources>> {
            use gfx_window_dxgi::update_views;

            let physical_size = new_size.to_physical(new_hidpi_factor);

            let (width, height): (u32, u32) = physical_size.into();
            let width = width as gfx::texture::Size;
            let height = height as gfx::texture::Size;

            match update_views(window, factory, device, width, height) {
                Ok(new_color) => {
                    let new_depth = factory
                        .create_depth_stencil_view_only(width, height)
                        .unwrap();

                    Some(WindowTargets {
                        color: new_color,
                        depth: new_depth,
                        size: new_size,
                        hidpi_factor: new_hidpi_factor,
                    })
                }
                Err(e) => {
                    // TODO: getting `gfx_window_any::backend] Resize failed: The RTV cannot be changed due to the references to it existing`, even on gfx cube example
                    error!("Resize failed: {}", e);
                    None
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod backend {
    use super::*;

    pub struct Backend {}

    impl super::Backend<Resources> for Backend {
        type Device = gfx_device_gl::Device;
        type Factory = gfx_device_gl::Factory;
        type CommandBuffer = gfx_device_gl::CommandBuffer;
        type Window = glutin::WindowedContext;

        fn init(
            window_builder: winit::WindowBuilder,
            events_loop: &winit::EventsLoop,
        ) -> BackendInit<Resources, Self> {
            #[cfg(target_os = "emscripten")]
            let gl_version = glutin::GlRequest::Specific(glutin::Api::WebGl, (2, 0));

            #[cfg(not(target_os = "emscripten"))]
            let gl_version = glutin::GlRequest::GlThenGles {
                opengl_version: (3, 2), // TODO: try more versions
                opengles_version: (2, 0),
            };

            let context = glutin::ContextBuilder::new()
                .with_gl(gl_version)
                // .with_gl_debug_flag(true)
                .with_vsync(true);

            let (window, mut device, mut factory, main_color, main_depth) =
                gfx_window_glutin::init::<ColorFormat, DepthFormat>(
                    window_builder,
                    context,
                    events_loop,
                )
                .expect("Failed to create window");

            let shade_lang = device.get_info().shading_language;

            // let backend = if shade_lang.is_embedded {
            //     shade::Backend::GlslEs(shade_lang)
            // } else {
            //     shade::Backend::Glsl(shade_lang)
            // };

            BackendInit {
                window,
                device,
                factory,
                main_color,
                main_depth,
            }
        }

        fn create_encoder(
            factory: &mut Self::Factory,
        ) -> gfx::Encoder<Resources, Self::CommandBuffer> {
            factory.create_command_buffer().into()
        }

        fn flush(
            encode: &mut gfx::Encoder<Resources, Self::CommandBuffer>,
            device: &mut Self::Device,
        ) {
            encode.flush(device);
        }

        fn get_winit_window(window: &Self::Window) -> &winit::Window {
            window
        }

        fn swap_buffers(window: &Self::Window) {
            window.swap_buffers().unwrap();
        }

        fn resize_swapchain(
            window: &mut Self::Window,
            factory: &mut Self::Factory,
            device: &mut Self::Device,
            new_size: winit::dpi::LogicalSize,
            new_hidpi_factor: f64,
        ) -> Option<WindowTargets<Resources>> {
            let physical_size = new_size.to_physical(new_hidpi_factor);

            window.resize(physical_size);
            let (new_color, new_depth) = gfx_window_glutin::new_views(&window);

            Some(WindowTargets {
                color: new_color,
                depth: new_depth,
                size: new_size,
                hidpi_factor: new_hidpi_factor,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn launch_test() {
        env_logger::init();

        let window_builder = winit::WindowBuilder::new().with_title("hello");
        TestApplication::run(window_builder);
    }
}
