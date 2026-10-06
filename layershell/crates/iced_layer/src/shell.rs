//! The event loop: sctk on one side, iced's runtime, user interfaces and
//! compositors on the other. It does what `iced_winit::run` does for winit,
//! for surfaces that are declared by the program instead of opened by it.
use crate::handle::Handle;
use crate::keys;
use crate::wl::{self, Env, Grab, SurfaceSettings, Wl};

use iced_core as core;
use iced_core::mouse;
use iced_core::renderer;
use iced_core::shell;
use iced_core::theme;
use iced_core::time::{Duration, Instant};
use iced_core::window;
use iced_core::{Event, PixelScaleMode, Point, Size, Widget};
use iced_futures::Executor;
use iced_futures::Runtime;
use iced_futures::futures::StreamExt;
use iced_futures::futures::channel::mpsc;
use iced_futures::futures::{Sink, task};
use iced_futures::subscription;
use iced_graphics::compositor::{self, Compositor as _};
use iced_graphics::{Shell, Viewport};
use iced_program::Program;
use iced_runtime::user_interface::{self, UserInterface};
use iced_runtime::{Action, Task};

use rustc_hash::FxHashMap;

use iced_core::Renderer as _;
use smithay_client_toolkit::reexports::calloop::channel;
use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopHandle};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::reexports::client::Connection;
use smithay_client_toolkit::reexports::client::Proxy as _;
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::seat::pointer::{
    BTN_BACK, BTN_FORWARD, BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, CursorIcon,
};

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::ptr::NonNull;
use std::rc::Rc;
use std::slice;

/// A program that says which layer surfaces it needs.
pub trait Layered: Program {
    /// The surfaces that exist while the program is in `state`. The shell
    /// opens the ones that are new, applies changed settings to the ones that
    /// persist, and closes the ones that are gone.
    ///
    /// `env` is what the compositor offers: the outputs there are (a bar per
    /// output is a loop over them) and whether a popup can hear a click
    /// outside of it.
    fn surfaces(&self, state: &Self::State, env: &Env) -> Vec<(window::Id, SurfaceSettings)>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not connect to the Wayland display: {0}")]
    Connect(String),
    #[error("the compositor is missing something: {0}")]
    Compositor(String),
    #[error("the event loop failed: {0}")]
    EventLoop(String),
    #[error("the futures executor could not be created")]
    Executor(#[from] iced_futures::futures::io::Error),
    #[error("the graphics context could not be created")]
    Graphics(#[from] core::backend::Error),
}

/// How long a surface may wait for a configure before it is drawn anyway.
const CONFIGURE_TIMEOUT: Duration = Duration::from_millis(300);

type Comp<P> = <<P as Program>::Renderer as compositor::Default>::Compositor;
type Surf<P> = <Comp<P> as compositor::Compositor>::Surface;

/// Tells the loop something happened, from any thread.
struct Proxy<T: 'static> {
    raw: channel::Sender<Action<T>>,
    sender: mpsc::UnboundedSender<Action<T>>,
}

impl<T: 'static> Clone for Proxy<T> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            sender: self.sender.clone(),
        }
    }
}

impl<T: Send + 'static> Proxy<T> {
    fn new(raw: channel::Sender<Action<T>>) -> (Self, impl Future<Output = ()> + Send + 'static) {
        let (sender, mut receiver) = mpsc::unbounded::<Action<T>>();
        let forward = raw.clone();

        let worker = async move {
            while let Some(action) = receiver.next().await {
                let _ = forward.send(action);
            }
        };

        (Self { raw, sender }, worker)
    }

    fn send_action(&self, action: Action<T>) {
        let _ = self.raw.send(action);
    }
}

impl<T: Send + 'static> Sink<Action<T>> for Proxy<T> {
    type Error = mpsc::SendError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.sender).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, action: Action<T>) -> Result<(), Self::Error> {
        Pin::new(&mut self.sender).start_send(action)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.sender).poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.sender).poll_close(cx)
    }
}

impl<T: Send + 'static> iced_graphics::shell::Notifier for Proxy<T> {
    fn tick(&self) {
        self.send_action(Action::Tick);
    }

    fn request_redraw(&self) {
        self.send_action(Action::Window(iced_runtime::window::Action::RedrawAll));
    }

    fn invalidate_layout(&self) {
        self.send_action(Action::Window(iced_runtime::window::Action::RelayoutAll));
    }
}

/// A program and its state.
struct Instance<P: Layered> {
    program: P,
    state: P::State,
}

impl<P: Layered> Instance<P> {
    fn new(program: P) -> (Self, Task<P::Message>) {
        let (state, task) = program.boot();

        (Self { program, state }, task)
    }

    fn update(&mut self, message: P::Message) -> Task<P::Message> {
        self.program.update(&mut self.state, message)
    }

    fn view(&self, window: window::Id) -> impl Widget<P::Message, P::Theme, P::Renderer> {
        self.program.view(&self.state, window)
    }

    fn subscription(&self) -> iced_futures::Subscription<P::Message> {
        self.program.subscription(&self.state)
    }

    fn theme(&self, window: window::Id) -> Option<P::Theme> {
        self.program.theme(&self.state, window)
    }

    fn style(&self, theme: &P::Theme) -> theme::Style {
        self.program.style(&self.state, theme)
    }

    fn surfaces(&self, env: &Env) -> Vec<(window::Id, SurfaceSettings)> {
        self.program.surfaces(&self.state, env)
    }
}

/// A surface with a renderer: what iced calls a window.
struct Window<P: Program>
where
    P::Theme: theme::Base,
{
    id: window::Id,
    handle: Handle,
    surface: Surf<P>,
    renderer: P::Renderer,
    viewport: Viewport,
    /// The physical size and scale the viewport was made for.
    synced: (Size<u32>, f64, u64),
    scale: f64,
    waker: shell::Waker,
    cache: Option<user_interface::Cache>,
    events: Vec<Event>,
    pointer: Option<(f64, f64)>,
    modifiers: core::keyboard::Modifiers,
    mouse_interaction: mouse::Interaction,
    dirty: bool,
    redraw_at: Option<Instant>,
    default_theme: P::Theme,
}

impl<P: Program> Window<P>
where
    P::Theme: theme::Base,
{
    fn cursor(&self) -> mouse::Cursor {
        match self.pointer {
            Some((x, y)) => mouse::Cursor::Available(self.layout_point((x, y))),
            None => mouse::Cursor::Unavailable,
        }
    }

    /// Surface-local logical pixels to the pixels the interface is laid out in.
    fn layout_point(&self, (x, y): (f64, f64)) -> Point {
        let factor = f64::from(self.viewport.scale_factor());

        Point::new(
            (x * self.scale / factor) as f32,
            (y * self.scale / factor) as f32,
        )
    }

    fn request_redraw(&mut self, request: window::RedrawRequest) {
        match request {
            window::RedrawRequest::NextFrame => {
                self.dirty = true;
                self.redraw_at = None;
            }
            window::RedrawRequest::At(at) => {
                self.redraw_at = Some(at);
            }
            window::RedrawRequest::Wait => {}
        }
    }

    /// The iced events a Wayland event stands for.
    fn convert(&mut self, event: wl::Event) -> Vec<Event> {
        use core::keyboard;

        match event {
            wl::Event::PointerEntered => vec![Event::Mouse(mouse::Event::CursorEntered)],
            wl::Event::PointerLeft => {
                self.pointer = None;

                vec![Event::Mouse(mouse::Event::CursorLeft)]
            }
            wl::Event::PointerMoved(position) => {
                self.pointer = Some(position);

                vec![Event::Mouse(mouse::Event::CursorMoved {
                    position: self.layout_point(position),
                })]
            }
            wl::Event::PointerButton { button, pressed } => {
                let button = match button {
                    BTN_LEFT => mouse::Button::Left,
                    BTN_RIGHT => mouse::Button::Right,
                    BTN_MIDDLE => mouse::Button::Middle,
                    BTN_BACK => mouse::Button::Back,
                    BTN_FORWARD => mouse::Button::Forward,
                    other => mouse::Button::Other(other as u16),
                };

                vec![Event::Mouse(if pressed {
                    mouse::Event::ButtonPressed(button)
                } else {
                    mouse::Event::ButtonReleased(button)
                })]
            }
            wl::Event::PointerScroll { x, y } => {
                // Wayland scrolls towards the content; iced, like winit,
                // towards the user.
                let delta = if x.value120 != 0 || y.value120 != 0 {
                    mouse::ScrollDelta::Lines {
                        x: -x.value120 as f32 / 120.0,
                        y: -y.value120 as f32 / 120.0,
                    }
                } else {
                    mouse::ScrollDelta::Pixels {
                        x: -x.absolute as f32,
                        y: -y.absolute as f32,
                    }
                };

                vec![Event::Mouse(mouse::Event::WheelScrolled { delta })]
            }
            wl::Event::KeyPressed { event, repeat } => {
                let text = event.utf8.as_deref();
                let key = keys::key(event.keysym, text);

                vec![Event::Keyboard(keyboard::Event::KeyPressed {
                    modified_key: key.clone(),
                    key,
                    physical_key: keys::physical(event.raw_code),
                    location: keys::location(event.keysym),
                    modifiers: self.modifiers,
                    text: keys::text(text),
                    repeat,
                })]
            }
            wl::Event::KeyReleased(event) => {
                let key = keys::key(event.keysym, None);

                vec![Event::Keyboard(keyboard::Event::KeyReleased {
                    modified_key: key.clone(),
                    key,
                    physical_key: keys::physical(event.raw_code),
                    location: keys::location(event.keysym),
                    modifiers: self.modifiers,
                })]
            }
            wl::Event::Modifiers(modifiers) => {
                self.modifiers = keys::modifiers(modifiers);

                vec![Event::Keyboard(keyboard::Event::ModifiersChanged(
                    self.modifiers,
                ))]
            }
            wl::Event::Focused(true) => vec![Event::Window(window::Event::Focused)],
            wl::Event::Focused(false) => vec![Event::Window(window::Event::Unfocused)],
            wl::Event::Changed | wl::Event::Closed => Vec::new(),
        }
    }
}

fn cursor_icon(interaction: mouse::Interaction) -> Option<CursorIcon> {
    use mouse::Interaction;

    Some(match interaction {
        Interaction::Hidden => return None,
        Interaction::None | Interaction::Idle => CursorIcon::Default,
        Interaction::ContextMenu => CursorIcon::ContextMenu,
        Interaction::Help => CursorIcon::Help,
        Interaction::Pointer => CursorIcon::Pointer,
        Interaction::Progress => CursorIcon::Progress,
        Interaction::Wait => CursorIcon::Wait,
        Interaction::Cell => CursorIcon::Cell,
        Interaction::Crosshair => CursorIcon::Crosshair,
        Interaction::Text => CursorIcon::Text,
        Interaction::Alias => CursorIcon::Alias,
        Interaction::Copy => CursorIcon::Copy,
        Interaction::Move => CursorIcon::Move,
        Interaction::NoDrop => CursorIcon::NoDrop,
        Interaction::NotAllowed => CursorIcon::NotAllowed,
        Interaction::Grab => CursorIcon::Grab,
        Interaction::Grabbing => CursorIcon::Grabbing,
        Interaction::ResizingHorizontally => CursorIcon::EwResize,
        Interaction::ResizingVertically => CursorIcon::NsResize,
        Interaction::ResizingDiagonallyUp => CursorIcon::NeswResize,
        Interaction::ResizingDiagonallyDown => CursorIcon::NwseResize,
        Interaction::ResizingColumn => CursorIcon::ColResize,
        Interaction::ResizingRow => CursorIcon::RowResize,
        Interaction::AllScroll => CursorIcon::AllScroll,
        Interaction::ZoomIn => CursorIcon::ZoomIn,
        Interaction::ZoomOut => CursorIcon::ZoomOut,
    })
}

fn viewport_for(physical: Size<u32>, scale: f64, mode: PixelScaleMode) -> Viewport {
    let pixel_scale = mode.resolve(scale as f32);

    if pixel_scale > 1 {
        Viewport::with_pixel_scale(physical, pixel_scale)
    } else {
        Viewport::with_physical_size(
            physical,
            renderer::Scale {
                window: scale as f32,
                application: 1.0,
            },
        )
    }
}

fn physical_size(logical: (u32, u32), scale: f64) -> Size<u32> {
    Size::new(
        (f64::from(logical.0) * scale).round().max(1.0) as u32,
        (f64::from(logical.1) * scale).round().max(1.0) as u32,
    )
}

// Large measuring overlays are transient. After the last declaration drops,
// return glibc's free arena pages as well as the Wayland pool's mappings.
fn release_raster(settings: Option<SurfaceSettings>) {
    let large = settings
        .as_ref()
        .and_then(|s| s.raster.as_ref())
        .is_some_and(|r| r.byte_len() >= 1024 * 1024);
    drop(settings);
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    if large {
        unsafe extern "C" {
            fn malloc_trim(pad: usize) -> i32;
        }
        // No live allocation is touched; only pages already free in the arena.
        let returned = unsafe { malloc_trim(0) };
        log::debug!("release raster: malloc_trim returned {returned}");
    }
}

fn retain_retries(
    closed: &mut HashMap<window::Id, Instant>,
    wanted: &HashMap<window::Id, SurfaceSettings>,
    now: Instant,
) {
    closed.retain(|id, at| {
        wanted.contains_key(id) && now.duration_since(*at) < Duration::from_millis(500)
    });
}

fn build_user_interface<'a, P: Layered>(
    instance: &'a Instance<P>,
    cache: user_interface::Cache,
    renderer: &mut P::Renderer,
    size: Size,
    id: window::Id,
) -> UserInterface<'a, P::Message, P::Theme, P::Renderer>
where
    P::Theme: theme::Base,
{
    UserInterface::build(instance.view(id), size, cache, renderer)
}

/// Runs the messages through the program, which gives back what its tasks
/// asked of the shell.
fn update<P: Layered, E: Executor>(
    instance: &mut Instance<P>,
    runtime: &mut Runtime<E, Proxy<P::Message>, Action<P::Message>>,
    bus: &mut shell::Bus<P::Message>,
) -> Vec<Action<P::Message>>
where
    P::Theme: theme::Base,
    P::Message: Send,
{
    use iced_futures::futures::task::{Context, Poll, noop_waker_ref};

    let mut actions = Vec::new();
    let mut outputs = Vec::new();

    while !bus.is_empty() {
        for (message, _receipt) in bus.drain() {
            let task = runtime.enter(|| instance.update(message));

            if let Some(mut stream) = iced_runtime::task::into_stream(task) {
                let mut context = Context::from_waker(noop_waker_ref());

                // Whatever is ready now is run now; the rest goes to the
                // executor.
                loop {
                    match runtime.enter(|| stream.poll_next_unpin(&mut context)) {
                        Poll::Ready(Some(Action::Output(output))) => outputs.push(output),
                        Poll::Ready(Some(action)) => actions.push(action),
                        Poll::Ready(None) => break,
                        Poll::Pending => {
                            runtime.run(stream);
                            break;
                        }
                    }
                }
            }
        }

        for output in outputs.drain(..) {
            let _ = bus.push(output);
        }
    }

    let subscription = runtime.enter(|| instance.subscription());

    runtime.track(subscription::into_recipes(subscription.map(Action::Output)));

    actions
}

pub fn run<P>(program: P) -> Result<(), Error>
where
    P: Layered + 'static,
    P::Theme: theme::Base,
    P::Message: Send + 'static,
{
    let settings = program.settings();
    let backend_settings = core::backend::Settings::from(&settings);
    let renderer_settings = renderer::Settings::from(&settings);
    let pixel_mode = settings.pixel_scale;
    let fonts = settings.fonts.clone();

    // Connecting and the first round trips: what globals exist, which outputs
    // there are and at what scale (xdg-output), what the seats can do.
    let conn = Connection::connect_to_env().map_err(|error| Error::Connect(error.to_string()))?;

    let (globals, mut event_queue) =
        registry_queue_init::<Wl>(&conn).map_err(|error| Error::Connect(error.to_string()))?;

    let qh = event_queue.handle();

    let mut event_loop: EventLoop<'static, Wl> =
        EventLoop::try_new().map_err(|error| Error::EventLoop(error.to_string()))?;

    let loop_handle: LoopHandle<'static, Wl> = event_loop.handle();

    let mut wl = Wl::new(&conn, &globals, &qh, loop_handle.clone()).map_err(Error::Compositor)?;

    for _ in 0..2 {
        event_queue
            .roundtrip(&mut wl)
            .map_err(|error| Error::Connect(error.to_string()))?;
    }

    log::info!("outputs: {:?}", wl.output_names());

    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle.clone())
        .map_err(|error| Error::EventLoop(error.to_string()))?;

    // What the runtime has to tell the loop arrives on a channel; the loop is
    // woken by it, and picks the actions up between two dispatches.
    let pending: Rc<RefCell<VecDeque<Action<P::Message>>>> = Rc::default();

    let (action_sender, action_receiver) = channel::channel::<Action<P::Message>>();

    {
        let pending = pending.clone();

        loop_handle
            .insert_source(action_receiver, move |event, _, _| {
                if let channel::Event::Msg(action) = event {
                    pending.borrow_mut().push_back(action);
                }
            })
            .map_err(|error| Error::EventLoop(error.to_string()))?;
    }

    let (proxy, worker) = Proxy::new(action_sender);

    let mut runtime = {
        let executor = P::Executor::new()?;
        executor.spawn(worker);

        Runtime::new(executor, proxy.clone())
    };

    let (mut instance, task) = runtime.enter(|| Instance::new(program));

    if let Some(stream) = iced_runtime::task::into_stream(task) {
        runtime.run(stream);
    }

    runtime.track(subscription::into_recipes(
        runtime.enter(|| instance.subscription().map(Action::Output)),
    ));

    let mut compositor: Option<Comp<P>> = None;
    let mut fonts_loaded = false;
    let mut windows: FxHashMap<window::Id, Window<P>> = FxHashMap::default();
    let mut bus = shell::Bus::new();
    let mut declared: HashMap<window::Id, SurfaceSettings> = HashMap::new();
    let mut closed_at: HashMap<window::Id, Instant> = HashMap::new();
    let mut actions: VecDeque<Action<P::Message>> = VecDeque::new();
    let mut running = true;
    let mut more_work = true;
    let mut stats = Stats::from_env();

    while running {
        // 1. Make the surfaces that exist the ones the program declares.
        let env = Env {
            outputs: wl.outputs(),
            focus_grab: wl.has_focus_grab(),
            placements: wl.placements(),
        };

        let wanted: HashMap<_, _> = instance.surfaces(&env).into_iter().collect();
        retain_retries(&mut closed_at, &wanted, Instant::now());

        for id in declared.keys().copied().collect::<Vec<_>>() {
            if !wanted.contains_key(&id) {
                log::info!("closing surface {id:?}");

                let _ = windows.remove(&id);
                wl.destroy_surface(id);
                release_raster(declared.remove(&id));
            }
        }

        for (id, settings) in wanted {
            match declared.get(&id) {
                None => {
                    if closed_at
                        .get(&id)
                        .is_some_and(|at| at.elapsed() < Duration::from_millis(500))
                    {
                        continue;
                    }

                    log::info!("opening surface {id:?}: {settings:?}");

                    wl.create_surface(&qh, id, settings.clone(), pixel_mode);
                    let _ = declared.insert(id, settings);
                }
                Some(current) if *current != settings => {
                    wl.update_surface(id, &settings);
                    let _ = declared.insert(id, settings);

                    if let Some(window) = windows.get_mut(&id) {
                        window.dirty = true;
                    }
                }
                Some(_) => {}
            }
        }

        // The focus grab holds the surfaces that are up (a surface that has
        // not been drawn yet is not mapped, and cannot be grabbed).
        {
            let up = |id: &window::Id| windows.contains_key(id);

            // The popups first: the compositor hands the keyboard to the first
            // surface of a grab, and that should not be the bar.
            let mut members: Vec<_> = declared
                .iter()
                .filter(|(id, settings)| settings.grab != Grab::None && up(id))
                .map(|(id, settings)| (settings.grab != Grab::Popup, *id))
                .collect();

            members.sort();

            let members: Vec<_> = members.into_iter().map(|(_, id)| id).collect();

            let popup = declared
                .iter()
                .any(|(id, settings)| settings.grab == Grab::Popup && up(id));

            wl.sync_grab(&qh, &members, popup);
        }

        // With no window left there is nothing to draw with: let the graphics
        // go, so that a host with nothing on screen holds nothing. It is made
        // again when the next window opens.
        if windows.is_empty() && compositor.is_some() {
            log::debug!("no window left: dropping the graphics");

            compositor = None;
        }

        // 2. Sleep until something happens: a Wayland event, an action from
        //    the runtime, or the next time a window wants to be redrawn.
        let timeout = if more_work {
            Some(Duration::ZERO)
        } else {
            let mut next: Option<Instant> = windows.values().filter_map(|w| w.redraw_at).min();

            let mut consider = |at: Instant| {
                next = Some(next.map_or(at, |next| next.min(at)));
            };

            for (id, surface) in &wl.surfaces {
                if !windows.contains_key(id)
                    && surface.configured.is_some()
                    && !surface.scale_known
                    && surface.created.elapsed() < wl::SCALE_GRACE
                {
                    consider(surface.created + wl::SCALE_GRACE);
                }

                if let Some(since) = surface.awaiting {
                    consider(since + CONFIGURE_TIMEOUT);
                }
            }

            for at in closed_at.values() {
                consider(*at + Duration::from_millis(500));
            }

            next.map(|at| at.saturating_duration_since(Instant::now()))
        };

        more_work = false;

        event_loop
            .dispatch(timeout, &mut wl)
            .map_err(|error| Error::EventLoop(error.to_string()))?;

        actions.extend(pending.borrow_mut().drain(..));

        // A click outside the popups of a focus grab: they are asked to close.
        if wl.take_grab_cleared() {
            for (id, settings) in &declared {
                if settings.grab == Grab::Popup {
                    if let Some(window) = windows.get_mut(id) {
                        window
                            .events
                            .push(Event::Window(window::Event::CloseRequested));
                    }
                }
            }
        }

        // 3. Wayland events: surfaces closed, resized, rescaled; input.
        for (id, event) in std::mem::take(&mut wl.events) {
            if matches!(event, wl::Event::Closed) {
                log::warn!("the compositor closed surface {id:?}");

                let _ = windows.remove(&id);
                wl.destroy_surface(id);
                release_raster(declared.remove(&id));
                let _ = closed_at.insert(id, Instant::now());

                continue;
            }

            let Some(window) = windows.get_mut(&id) else {
                continue;
            };

            let events = window.convert(event);

            for event in events {
                window.events.push(event);
            }
        }

        // A surface that was asked for a size and has not heard back is shown
        // anyway after a while: a compositor is not obliged to answer a
        // request that changes nothing.
        for surface in wl.surfaces.values_mut() {
            if surface
                .awaiting
                .is_some_and(|since| since.elapsed() >= CONFIGURE_TIMEOUT)
            {
                log::debug!("no configure after a request; going on");
                surface.awaiting = None;
            }
        }

        // 4. Settle the geometry of every surface and open the ones that can
        //    be.
        let ids: Vec<_> = wl.surfaces.keys().copied().collect();

        for id in ids {
            wl.sync_geometry(id, pixel_mode);

            let Some(surface) = wl.surfaces.get(&id) else {
                continue;
            };

            if !surface.is_ready() {
                continue;
            }

            if surface.settings.raster.is_some() {
                if let Err(error) = wl.present_raster(id) {
                    log::warn!("could not present raster {id:?}: {error}");
                }
                continue;
            }

            let logical = surface.configured.expect("A ready surface is configured");
            let scale = surface.scale;
            let physical = physical_size(logical, scale);

            if let Some(window) = windows.get_mut(&id) {
                let key = (physical, scale, surface.version);

                if window.synced != key {
                    let moved = window.synced.0 != physical;

                    window.viewport = viewport_for(physical, scale, pixel_mode);
                    window.scale = scale;
                    window.synced = key;
                    window.renderer.hint(window.viewport.target().scale());

                    if moved {
                        if let Some(compositor) = compositor.as_mut() {
                            compositor.configure_surface(
                                &mut window.surface,
                                physical.width,
                                physical.height,
                            );
                        }
                    }

                    window.events.push(Event::Window(window::Event::Resized(
                        window.viewport.logical_size(),
                    )));
                    window.dirty = true;
                }

                continue;
            }

            // A new window.
            let Some(surface_ptr) = NonNull::new(surface.wl_surface().id().as_ptr().cast()) else {
                continue;
            };

            let Some(display_ptr) = NonNull::new(wl.display_ptr()) else {
                continue;
            };

            let handle = Handle::new(display_ptr, surface_ptr);

            if compositor.is_none() {
                let shell = Shell::new(proxy.clone());
                let backend_settings = backend_settings.clone();

                let created = runtime.block_on(<Comp<P> as compositor::Compositor>::new(
                    backend_settings,
                    handle,
                    handle,
                    shell,
                ))?;

                let mut created = created;

                // The font system outlives the compositor, so the fonts go in
                // once, not once for every time the compositor is made again.
                if !fonts_loaded {
                    for font in &fonts {
                        let _ = created.load_font(font.clone());
                    }

                    fonts_loaded = true;
                }

                log::info!("graphics: {:?}", created.information());

                compositor = Some(created);
            }

            let compositor = compositor.as_mut().expect("The compositor exists");
            let viewport = viewport_for(physical, scale, pixel_mode);
            let surface_ = compositor.create_surface(handle, physical.width, physical.height);
            let mut renderer = compositor.create_renderer(renderer_settings);

            renderer.hint(viewport.target().scale());

            let waker = {
                let proxy = proxy.clone();

                shell::Waker::new(move || {
                    proxy.send_action(Action::Event {
                        window: id,
                        event: Event::Waken,
                    });
                })
            };

            log::info!(
                "surface {id:?} is up: {}x{} logical, {}x{} physical at {scale:.4}, \
                 pixel scale {}, {}x{} virtual pixels",
                logical.0,
                logical.1,
                physical.width,
                physical.height,
                viewport.pixel_scale(),
                viewport.logical_size().width,
                viewport.logical_size().height,
            );

            let _ = windows.insert(
                id,
                Window {
                    id,
                    handle,
                    surface: surface_,
                    renderer,
                    synced: (physical, scale, wl.surfaces[&id].version),
                    scale,
                    viewport: viewport.clone(),
                    waker,
                    cache: Some(user_interface::Cache::default()),
                    events: vec![Event::Window(window::Event::Opened {
                        position: None,
                        size: viewport.logical_size(),
                        scale_factor: scale as f32,
                    })],
                    pointer: None,
                    modifiers: core::keyboard::Modifiers::empty(),
                    mouse_interaction: mouse::Interaction::None,
                    dirty: true,
                    redraw_at: None,
                    default_theme: <P::Theme as theme::Base>::default(theme::Mode::None),
                },
            );
        }

        // 5. Actions the runtime asked for.
        while let Some(action) = actions.pop_front() {
            match action {
                Action::Output(message) => {
                    let _ = bus.push(message);
                }
                Action::Widget(operation) => {
                    let mut current = Some(operation);

                    while let Some(mut operation) = current.take() {
                        for (id, window) in windows.iter_mut() {
                            let cache = window.cache.take().unwrap_or_default();
                            let size = window.viewport.logical_size();

                            let mut ui = build_user_interface(
                                &instance,
                                cache,
                                &mut window.renderer,
                                size,
                                *id,
                            );

                            ui.operate(&window.renderer, operation.as_mut());
                            window.cache = Some(ui.into_cache());
                        }

                        if let core::widget::operation::Outcome::Chain(next) = operation.finish() {
                            current = Some(next);
                        }
                    }

                    for window in windows.values_mut() {
                        window.dirty = true;
                    }
                }
                Action::Window(action) => match action {
                    iced_runtime::window::Action::RedrawAll
                    | iced_runtime::window::Action::RelayoutAll => {
                        for window in windows.values_mut() {
                            window.dirty = true;
                        }
                    }
                    _ => log::debug!("ignoring a window action: windows are declared, not opened"),
                },
                Action::Font(action) => match action {
                    iced_runtime::font::Action::Load { bytes, channel } => {
                        if let Some(compositor) = compositor.as_mut() {
                            let _ = channel.send(compositor.load_font(bytes));
                        }
                    }
                    iced_runtime::font::Action::List { channel } => {
                        if let Some(compositor) = compositor.as_mut() {
                            let _ = channel.send(compositor.list_fonts());
                        }
                    }
                    iced_runtime::font::Action::SetDefaults { .. } => {}
                },
                Action::System(action) => {
                    if let iced_runtime::system::Action::GetTheme(channel) = action {
                        let _ = channel.send(theme::Mode::None);
                    }
                }
                Action::Event { window, event } => {
                    if let Some(window) = windows.get_mut(&window) {
                        window.events.push(event);
                    }
                }
                Action::Tick => {
                    for window in windows.values_mut() {
                        window.renderer.tick();
                    }
                }
                Action::Reload => {
                    for window in windows.values_mut() {
                        window.dirty = true;
                    }
                }
                Action::Exit => running = false,
                Action::Clipboard(_) | Action::Image(_) | Action::Backend(_) => {
                    log::debug!("ignoring an action the layer shell does not support");
                }
            }
        }

        if !running {
            break;
        }

        // 6. Input: each window's interface sees what happened to it.
        let mut stale = false;

        for (id, window) in windows.iter_mut() {
            if window.events.is_empty() {
                continue;
            }

            let interacting = Instant::now();
            let id = *id;
            let events = std::mem::take(&mut window.events);
            let cursor = window.cursor();
            let size = window.viewport.logical_size();
            let cache = window.cache.take().unwrap_or_default();

            let mut ui = build_user_interface(&instance, cache, &mut window.renderer, size, id);

            let (state, statuses) = ui.update(
                &window.handle,
                &window.waker,
                &events,
                cursor,
                &mut window.renderer,
                &mut bus,
            );

            window.cache = Some(ui.into_cache());

            match state {
                user_interface::State::Updated {
                    redraw_request,
                    mouse_interaction,
                    ..
                } => {
                    window.request_redraw(redraw_request);
                    window.mouse_interaction = mouse_interaction;

                    wl.set_cursor(id, cursor_icon(mouse_interaction));
                }
                user_interface::State::Outdated => stale = true,
            }

            for (event, status) in events.into_iter().zip(statuses) {
                runtime.broadcast(subscription::Event::Interaction {
                    window: id,
                    event,
                    status,
                });
            }

            if stats.enabled {
                stats.interact.push(interacting.elapsed());
            }
        }

        // 7. Messages become state, and state becomes new interfaces.
        if !bus.is_empty() || stale {
            let produced = update(&mut instance, &mut runtime, &mut bus);

            actions.extend(produced);

            for window in windows.values_mut() {
                if wl
                    .surfaces
                    .get(&window.id)
                    .is_none_or(|surface| surface.settings.repaint_revision.is_none())
                {
                    window.dirty = true;
                }
            }

            more_work = true;

            continue;
        }

        // 8. Draw whatever wants it.
        let now = Instant::now();

        let due: Vec<_> = windows
            .iter()
            .filter(|(id, window)| {
                let ready = wl.surfaces.get(id).is_some_and(wl::Surface::is_ready);

                ready && (window.dirty || window.redraw_at.is_some_and(|at| at <= now))
            })
            .map(|(id, _)| *id)
            .collect();

        for id in due {
            let Some(compositor) = compositor.as_mut() else {
                break;
            };

            let Some(window) = windows.get_mut(&id) else {
                continue;
            };

            window.dirty = false;
            window.redraw_at = None;

            let redrawn = redraw(
                &mut instance,
                &mut runtime,
                &mut bus,
                &mut actions,
                compositor,
                &mut wl,
                window,
                &mut stats,
            );

            if redrawn.messages {
                for other in windows.values_mut() {
                    other.dirty = true;
                }

                more_work = true;
            }
        }

        if !actions.is_empty() {
            more_work = true;
        }
    }

    // The renderer's surfaces must go before the Wayland surfaces they
    // draw to.
    windows.clear();
    drop(compositor);

    stats.report();

    Ok(())
}

/// Timings of what drawing costs, kept when `ICED_LAYER_STATS` is set and
/// printed when the shell exits.
#[derive(Default)]
struct Stats {
    enabled: bool,
    started: Option<Instant>,
    first_frame: Option<Duration>,
    interact: Vec<Duration>,
    prepare: Vec<Duration>,
    draw: Vec<Duration>,
    present: Vec<Duration>,
}

impl Stats {
    fn from_env() -> Self {
        Self {
            enabled: std::env::var_os("ICED_LAYER_STATS").is_some(),
            started: Some(Instant::now()),
            ..Self::default()
        }
    }

    fn report(&self) {
        if !self.enabled {
            return;
        }

        fn summary(name: &str, samples: &[Duration]) {
            if samples.is_empty() {
                return;
            }

            let mut sorted = samples.to_vec();
            sorted.sort();

            let total: Duration = sorted.iter().sum();
            let micros = |d: Duration| d.as_secs_f64() * 1e6;

            eprintln!(
                "stats {name:<9} n={:<5} mean={:>8.1}us p50={:>8.1}us p95={:>8.1}us max={:>8.1}us",
                sorted.len(),
                micros(total) / sorted.len() as f64,
                micros(sorted[sorted.len() / 2]),
                micros(sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)]),
                micros(*sorted.last().expect("Not empty")),
            );
        }

        if let Some(first) = self.first_frame {
            eprintln!(
                "stats first frame presented {:.1} ms after the shell started",
                first.as_secs_f64() * 1e3
            );
        }

        summary("interact", &self.interact);
        summary("prepare", &self.prepare);
        summary("draw", &self.draw);
        summary("present", &self.present);
    }
}

struct Redrawn {
    /// An update ran while drawing, so the other windows are out of date.
    messages: bool,
}

/// What `RedrawRequested` does in iced_winit: lets widgets react to the frame
/// (hover, animations), rebuilds if that produced messages, draws and
/// presents.
fn redraw<P: Layered>(
    instance: &mut Instance<P>,
    runtime: &mut Runtime<P::Executor, Proxy<P::Message>, Action<P::Message>>,
    bus: &mut shell::Bus<P::Message>,
    actions: &mut VecDeque<Action<P::Message>>,
    compositor: &mut Comp<P>,
    wl: &mut Wl,
    window: &mut Window<P>,
    stats: &mut Stats,
) -> Redrawn
where
    P::Theme: theme::Base,
    P::Message: Send + 'static,
{
    let id = window.id;
    let started = Instant::now();
    let size = window.viewport.logical_size();
    let cursor = window.cursor();
    let redraw_event = Event::Window(window::Event::RedrawRequested(Instant::now()));
    let mut messages = false;

    let cache = window.cache.take().unwrap_or_default();
    let mut interface = build_user_interface(instance, cache, &mut window.renderer, size, id);
    let mut redraw_count = 0;

    let state = loop {
        let message_count = bus.len();

        let (state, _) = interface.update(
            &window.handle,
            &window.waker,
            slice::from_ref(&redraw_event),
            cursor,
            &mut window.renderer,
            bus,
        );

        if redraw_count >= 2 {
            log::warn!("3 consecutive RedrawRequested events produced invalidation");

            break state;
        }

        if message_count == bus.len() {
            match &state {
                user_interface::State::Outdated => {}
                user_interface::State::Updated { change, .. } => match change {
                    user_interface::Change::None => break state,
                    user_interface::Change::Overlay => {
                        redraw_count += 1;
                        continue;
                    }
                    user_interface::Change::Layout => {}
                },
            }
        }

        redraw_count += 1;

        if !bus.is_empty() || matches!(state, user_interface::State::Outdated) {
            let cache = interface.into_cache();

            actions.extend(update(instance, runtime, bus));
            messages = true;

            interface = build_user_interface(instance, cache, &mut window.renderer, size, id);
        }
    };

    let theme = instance.theme(id);
    let theme = theme.as_ref().unwrap_or(&window.default_theme);
    let style = instance.style(theme);

    let prepared = Instant::now();

    interface.draw(
        &mut window.renderer,
        theme,
        &renderer::Style {
            text_color: style.text_color,
        },
        cursor,
    );

    let drawn = Instant::now();

    if let user_interface::State::Updated {
        redraw_request,
        mouse_interaction,
        ..
    } = state
    {
        window.request_redraw(redraw_request);
        window.mouse_interaction = mouse_interaction;

        wl.set_cursor(id, cursor_icon(mouse_interaction));
    }

    runtime.broadcast(subscription::Event::Interaction {
        window: id,
        event: redraw_event,
        status: core::event::Status::Ignored,
    });

    window.cache = Some(interface.into_cache());

    if let Some(surface) = wl.surfaces.get(&id) {
        if let Some(logical) = surface.configured {
            wl.set_destination(id, logical);
        }
    }

    let presenting = Instant::now();

    let result = compositor.present(
        &mut window.renderer,
        &mut window.surface,
        &window.viewport,
        style.background_color,
        || {},
    );

    if stats.enabled {
        if stats.first_frame.is_none() {
            stats.first_frame = stats.started.map(|at| at.elapsed());
        }

        stats.prepare.push(prepared - started);
        stats.draw.push(drawn - prepared);
        stats.present.push(presenting.elapsed());
    }

    match result {
        Ok(()) => {}
        Err(compositor::SurfaceError::Outdated | compositor::SurfaceError::Lost) => {
            let physical = window.viewport.physical_size();

            compositor.configure_surface(&mut window.surface, physical.width, physical.height);
            window.dirty = true;
        }
        Err(error) => log::warn!("could not present {id:?}: {error:?}"),
    }

    Redrawn { messages }
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn expired_or_removed_surfaces_leave_no_retry_wake() {
        let now = Instant::now();
        let young = window::Id::unique();
        let expired = window::Id::unique();
        let gone = window::Id::unique();
        let mut closed = HashMap::from([
            (young, now - Duration::from_millis(499)),
            (expired, now - Duration::from_millis(500)),
            (gone, now),
        ]);
        let wanted = HashMap::from([
            (young, SurfaceSettings::default()),
            (expired, SurfaceSettings::default()),
        ]);
        retain_retries(&mut closed, &wanted, now);
        assert_eq!(closed.len(), 1);
        assert!(closed.contains_key(&young));
        retain_retries(&mut closed, &wanted, now + Duration::from_millis(1));
        assert!(closed.is_empty());
    }
}
