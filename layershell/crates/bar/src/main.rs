//! A panel host: a bar on every output, and panels summoned over a socket,
//! drawn by quadrille on layer-shell surfaces.
mod graphics;
mod host;
mod ipc;
mod sys;
mod sysmon;
mod theme;

use host::{Host, Options};

use iced_core::Backend;
use iced_core::time::Duration;

const USAGE: &str = "\
usage: quadrille-bar [OPTIONS]
       quadrille-bar ctl COMMAND

options:
  --no-bar, --panels-only  no bar: no surface, no exclusive zone and no timer
                         until a panel is summoned (a service beside another bar)
  --output NAME          a bar on this output only; repeat for several (default: all)
  --backend NAME         tiny-skia (the default here) or wgpu
  --bar-tick-ms MS       how often the bar's gauges are read; 0 never (2000)
  --no-exclusive         the bar claims no space at the top of the output
  --height VPX           the bar's height in virtual pixels (25)
  --keyboard-exclusive   a panel takes the keyboard instead of a focus grab
  --passive              panels take no keyboard and no focus grab (screenshots)
  --theme-dir DIR        where the Omarchy theme is (~/.local/state/omarchy/current)
  --open PANEL           show a panel at the start
  --exit-after SECS      quit after a while (for tests)

commands (quadrille-bar ctl ...):
  summon PANEL [JSON]    show a panel; JSON may say {\"output\":\"eDP-2\"}
  toggle PANEL [JSON]    show it, or hide it if it is shown
  hide [PANEL|all]       hide the panel that is shown
  list                   the panels, the outputs and the theme
  reload-theme           read the Omarchy theme again
  quit";

fn parse() -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));

        match arg.as_str() {
            "--output" => options.outputs.push(value("--output")?),
            "--backend" => options.backend = Some(value("--backend")?),
            "--bar-tick-ms" => {
                options.bar_tick_ms = value("--bar-tick-ms")?
                    .parse()
                    .map_err(|_| "--bar-tick-ms needs a number")?
            }
            "--no-bar" | "--panels-only" => options.no_bar = true,
            "--no-exclusive" => options.exclusive = false,
            "--height" => {
                options.height = value("--height")?
                    .parse()
                    .map_err(|_| "--height needs a number")?
            }
            "--keyboard-exclusive" => options.exclusive_keyboard = true,
            "--passive" => options.passive = true,
            "--theme-dir" => options.theme_dir = value("--theme-dir")?.into(),
            "--open" => options.open = Some(value("--open")?),
            "--exit-after" => {
                options.exit_after = Some(Duration::from_secs_f64(
                    value("--exit-after")?
                        .parse()
                        .map_err(|_| "--exit-after needs seconds")?,
                ))
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown option {other}")),
        }
    }

    Ok(options)
}

fn main() {
    env_logger::init();

    let mut args = std::env::args().skip(1);

    if args.next().as_deref() == Some("ctl") {
        let command: Vec<String> = args.collect();

        match ipc::send(&command.join(" ")) {
            Ok(answer) => {
                print!("{answer}");

                if answer.starts_with("error:") {
                    std::process::exit(1);
                }
            }
            Err(error) => {
                let path = ipc::socket_path();

                let why = match error.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("there is no socket at {}", path.display())
                    }
                    std::io::ErrorKind::ConnectionRefused => {
                        format!(
                            "nothing listens at {}: it is left from a host that died",
                            path.display()
                        )
                    }
                    _ => format!("{error} at {}", path.display()),
                };

                eprintln!(
                    "quadrille-bar: the host is not running ({why}).\n\
                     Start it with `quadrille-bar --no-bar` (panels only) or `quadrille-bar` (bars and panels)."
                );
                std::process::exit(1);
            }
        }

        return;
    }

    let options = match parse() {
        Ok(options) => options,
        Err(error) => {
            if !error.is_empty() {
                eprintln!("quadrille-bar: {error}");
            }

            eprintln!("{USAGE}");
            std::process::exit(if error.is_empty() { 0 } else { 2 });
        }
    };

    if ipc::is_running() {
        eprintln!("quadrille-bar is already running");
        std::process::exit(1);
    }

    // quadrille::settings(), without reading every font on the machine yet.
    let mut settings = graphics::settings();

    // A panel is a few hundred thousand pixels that change once a second: the
    // CPU is the right place to draw them.
    settings.backend = Backend::Custom(
        options
            .backend
            .clone()
            .unwrap_or_else(|| "tiny-skia".into()),
    );

    let boot_options = std::cell::RefCell::new(Some(options));

    let result = iced_layer::application(
        move || Host::new(boot_options.borrow_mut().take().expect("Boot once")),
        Host::update,
        |host: &Host, window| host.view(window),
        |host: &Host, env| host.surfaces(env),
    )
    .settings(settings)
    .subscription(Host::subscription)
    .theme(|host: &Host, _| host.theme())
    .run();

    let _ = std::fs::remove_file(ipc::socket_path());

    if let Err(error) = result {
        eprintln!("quadrille-bar: {error}");
        std::process::exit(1);
    }
}
