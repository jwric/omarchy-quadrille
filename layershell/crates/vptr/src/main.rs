//! Refuses to touch anything but a nested compositor (see `main`).
//!
//! `vptr OUTPUT move X Y [click [left|right]] [sleep MS] [scroll DY] ...`
//!
//! X and Y are logical pixels on the output. Used to drive the spike on a live
//! compositor, since nothing else here can click.
use std::time::{Duration, Instant};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_output, wl_pointer, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

#[derive(Default)]
struct State {
    names: Vec<(wl_output::WlOutput, String, (i32, i32))>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let entry = match state.names.iter_mut().find(|(o, _, _)| o == output) {
            Some(entry) => entry,
            None => {
                state.names.push((output.clone(), String::new(), (0, 0)));
                state.names.last_mut().unwrap()
            }
        };

        if let wl_output::Event::Name { name } = event {
            entry.1 = name;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerManagerV1,
        _: <ZwlrVirtualPointerManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrVirtualPointerV1,
        _: <ZwlrVirtualPointerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn main() {
    // This moves a pointer. It must never be the real one: the display has to
    // be a nested compositor's, and tools/nested.sh says so by setting
    // QUADRILLE_NESTED to its name.
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();

    if display.is_empty() || std::env::var("QUADRILLE_NESTED").as_deref() != Ok(display.as_str()) {
        eprintln!(
            "vptr: refusing to run: WAYLAND_DISPLAY={display:?} is not marked as a nested \
             compositor (QUADRILLE_NESTED). Use tools/nested.sh run vptr ..."
        );
        std::process::exit(3);
    }

    let mut args = std::env::args().skip(1);
    let output_name = args.next().expect("usage: vptr OUTPUT COMMAND...");
    let commands: Vec<String> = args.collect();

    let conn = Connection::connect_to_env().expect("connect");
    let (globals, mut queue) = registry_queue_init::<State>(&conn).expect("registry");
    let qh = queue.handle();
    let mut state = State::default();

    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=1, ()).expect("seat");
    let manager: ZwlrVirtualPointerManagerV1 =
        globals.bind(&qh, 1..=2, ()).expect("virtual pointer");

    for global in globals.contents().clone_list() {
        if global.interface == "wl_output" {
            let _: wl_output::WlOutput =
                globals
                    .registry()
                    .bind(global.name, global.version.min(4), &qh, ());
        }
    }

    queue.roundtrip(&mut state).expect("roundtrip");
    queue.roundtrip(&mut state).expect("roundtrip");

    let output = state
        .names
        .iter()
        .find(|(_, name, _)| *name == output_name)
        .map(|(output, _, _)| output.clone())
        .unwrap_or_else(|| {
            panic!(
                "no output {output_name}: {:?}",
                state.names.iter().map(|n| &n.1).collect::<Vec<_>>()
            )
        });

    let pointer = manager.create_virtual_pointer_with_output(Some(&seat), Some(&output), &qh, ());

    // The extents are the output's logical size; ask for them on the command line.
    let (width, height) = match std::env::var("VPTR_EXTENT") {
        Ok(extent) => {
            let (w, h) = extent.split_once('x').expect("VPTR_EXTENT=WxH");
            (w.parse::<u32>().unwrap(), h.parse::<u32>().unwrap())
        }
        Err(_) => (1536, 960),
    };

    let start = Instant::now();
    let now = || start.elapsed().as_millis() as u32;

    let mut commands = commands.into_iter();

    while let Some(command) = commands.next() {
        match command.as_str() {
            "move" => {
                let x: u32 = commands.next().unwrap().parse().unwrap();
                let y: u32 = commands.next().unwrap().parse().unwrap();

                pointer.motion_absolute(now(), x, y, width, height);
                pointer.frame();
            }
            "click" => {
                let button = match commands.clone().next().as_deref() {
                    Some("right") => {
                        let _ = commands.next();
                        0x111
                    }
                    Some("left") => {
                        let _ = commands.next();
                        0x110
                    }
                    _ => 0x110,
                };

                pointer.button(now(), button, wl_pointer::ButtonState::Pressed);
                pointer.frame();
                queue.roundtrip(&mut state).unwrap();
                std::thread::sleep(Duration::from_millis(60));
                pointer.button(now(), button, wl_pointer::ButtonState::Released);
                pointer.frame();
            }
            "scroll" => {
                let dy: f64 = commands.next().unwrap().parse().unwrap();

                pointer.axis(now(), wl_pointer::Axis::VerticalScroll, dy);
                pointer.frame();
            }
            "sleep" => {
                let ms: u64 = commands.next().unwrap().parse().unwrap();

                queue.roundtrip(&mut state).unwrap();
                std::thread::sleep(Duration::from_millis(ms));
            }
            other => panic!("unknown command {other}"),
        }

        queue.roundtrip(&mut state).unwrap();
    }

    pointer.destroy();
    queue.roundtrip(&mut state).unwrap();
}
