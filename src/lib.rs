#![deny(clippy::all)]
#![warn(clippy::pedantic)]
// #![deny(missing_docs)]

use gfx::Device;

#[cfg(not(target_os = "windows"))]
pub type ColorFormat = gfx::format::Rgba8;
#[cfg(target_os = "windows")]
pub type ColorFormat = gfx::format::Bgra8; // TODO i thinkg this is right

pub type DepthFormat = gfx::format::DepthStencil;

#[cfg(target_os = "windows")]
pub type Resources = gfx_device_dx11::Resources;

#[cfg(not(target_os = "windows"))]
pub type Resources = gfx_device_gl::Resources;

#[cfg(not(target_os = "windows"))]
mod types {
    pub type Resources = gfx_device_gl::Resources;
}

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
    fn new(factory: &mut impl gfx::Factory<R>, window_targets: WindowTargets<R>) -> Self;

    fn on_event(&mut self, event: winit::Event);

    fn run(window_builder: winit::WindowBuilder) {
        #[cfg(not(target_os = "windows"))]
        run_loop(window_builder);

        #[cfg(target_os = "windows")]
        run_loop(window_builder);
    }
}

trait Application2<R, C>
where
    R: gfx::Resources,
    C: gfx::CommandBuffer<R>,
{
    fn new(factory: &mut impl gfx::Factory<R>, window_targets: WindowTargets<R>) -> Self;

    fn on_event(&mut self, event: winit::Event);

    fn run(window_builder: winit::WindowBuilder) {
        #[cfg(not(target_os = "windows"))]
        // run_loop(window_builder, GlHelper {});
        #[cfg(target_os = "windows")]
        run_loop(window_builder);
    }
}

// pub struct Wrap<R: gfx::Resources, C, A> {
//     encoder: gfx::Encoder<R, C>,
//     app: A,
// }

// impl<R, C, A> Application2<R, C> for Wrap<R, C, A>
// where
//     R: gfx::Resources,
//     C: gfx::CommandBuffer<R>,
//     A: Application<R>,
// {
// }

fn run_loop<A, R, H>(window_builder: winit::WindowBuilder)
where
    A: Sized + Application<R>,
    R: gfx::Resources,
    H: Helper<R>,
{
    let mut events_loop = winit::EventsLoop::new();

    let (window, mut device, mut factory, main_color, main_depth) =
        H::create_window(window_builder, &events_loop);

    let window_targets = WindowTargets {
        color: main_color,
        depth: main_depth,
        size: window.get_inner_size().unwrap(),
        hidpi_factor: window.get_hidpi_factor(),
    };

    let mut app = A::new(&mut factory, window_targets);

    let mut running = true;
    while running {
        events_loop.poll_events(|event| match event {
            _ => app.on_event(event),
        });

        // app.render();
        H::end_frame(&window);
        device.cleanup();
    }
}

trait Helper<R: gfx::Resources> {
    // fn new() -> Self;
    // fn factory(&self) -> &mut Self::Factory;

    type Device: gfx::Device;
    type Factory: gfx::Factory<R>;
    type CommandBuffer: gfx::CommandBuffer<R>;
    type Window: std::ops::Deref<Target = winit::Window>;

    fn create_window(
        window_builder: winit::WindowBuilder,
        events_loop: &winit::EventsLoop,
    ) -> (
        Self::Window,
        Self::Device,
        Self::Factory,
        gfx::handle::RenderTargetView<R, ColorFormat>,
        gfx::handle::DepthStencilView<R, DepthFormat>,
    );

    fn end_frame(window: &Self::Window);
}

struct GlHelper {}

impl Helper<Resources> for GlHelper {
    type Device = gfx_device_gl::Device;
    type Factory = gfx_device_gl::Factory;
    type CommandBuffer = gfx_device_gl::CommandBuffer;
    type Window = glutin::WindowedContext;

    fn create_window(
        window_builder: winit::WindowBuilder,
        events_loop: &winit::EventsLoop,
    ) -> (
        Self::Window,
        Self::Device,
        Self::Factory,
        gfx::handle::RenderTargetView<Resources, ColorFormat>,
        gfx::handle::DepthStencilView<Resources, DepthFormat>,
    ) {
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

        (window, device, factory, main_color, main_depth)
    }

    fn end_frame(window: &Self::Window) {
        window.swap_buffers().unwrap();
    }
}

struct H<R, F>
where
    R: gfx::Resources,
    F: gfx::Factory<R>,
{
    factory: F,
    phantom: std::marker::PhantomData<R>,
}

#[cfg(not(target_os = "windows"))]
impl H<gfx_device_gl::Resources, gfx_device_gl::Factory> {
    pub fn new() -> Self {
        unimplemented!()
    }
}
