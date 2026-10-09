//! A screensaver of technical drawings: sheets that plot themselves, set
//! their subject moving and document it part by part, one subject after
//! another, on every output, in the Omarchy theme.
mod display;
mod draft;
mod headless;
mod machine;
mod saver;
mod sheet;
mod subjects;

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use iced_core::Backend;
use quadrille_desktop::theme;

use headless::{Output, Stage, Studio};
use machine::{Fixture, Machine};
use saver::{Options, Saver};
use subjects::Subject;

#[derive(Parser)]
#[command(about = "Technical drawings that plot themselves while the desktop is idle")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// The computer the sheets of this computer draw.
    #[arg(long, global = true, value_enum, default_value_t = Source::Live)]
    machine: Source,

    #[command(flatten)]
    run: Run,
}

#[derive(Subcommand)]
enum Command {
    /// Draw sheets headless to PNG, exactly as an output would show them.
    Render(Render),
    /// Time drawing frames headless: a sheet's plot and its documented run.
    Bench(Bench),
    /// The subjects there are.
    List,
}

#[derive(Args)]
struct Bench {
    #[arg(long, value_enum, default_value_t = Desk::Laptop)]
    output: Desk,
    /// Frames a second to step at.
    #[arg(long, default_value_t = 30.0)]
    fps: f32,
}

#[derive(Args)]
struct Run {
    /// The layer surfaces' namespace; also what names the process to the
    /// desktop's tools, and its single-instance lock.
    #[arg(long, default_value = "quadrille-screensaver")]
    namespace: String,
    /// The subject to start with.
    #[arg(long)]
    subject: Option<String>,
    /// Frames a second.
    #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u32).range(1..=240))]
    fps: u32,
    /// What the order of the subjects is drawn from (default: the time).
    #[arg(long)]
    seed: Option<u64>,
    /// Input this many milliseconds after starting is ignored.
    #[arg(long, default_value_t = 1000)]
    grace: u64,
    /// The renderer: tiny-skia or wgpu.
    #[arg(long, default_value = "tiny-skia")]
    backend: String,
    /// Where the Omarchy theme is.
    #[arg(long, default_value_os_t = theme::default_dir())]
    theme_dir: PathBuf,
}

#[derive(Args)]
struct Render {
    /// One subject (default: each in turn).
    #[arg(long)]
    subject: Option<String>,
    /// Moments into the sheet, in seconds.
    #[arg(long, value_delimiter = ',', default_value = "4,12,16")]
    at: Vec<f32>,
    #[arg(long, value_enum, default_value_t = Desk::Laptop)]
    output: Desk,
    /// Another output than the desk's, by its mode:
    /// `WIDTHxHEIGHT[@SCALE][:MM]`, MM its panel's width (guessed when left
    /// out), like `1366x768` or `2880x1800@2:302`.
    #[arg(long, conflicts_with = "output")]
    size: Option<Output>,
    /// One of quadrille's themes (default: the current Omarchy theme).
    #[arg(long)]
    theme: Option<String>,
    /// At the output's resolution, not one pixel per virtual pixel.
    #[arg(long)]
    physical: bool,
    #[arg(long, default_value = "target/screensaver")]
    out: PathBuf,
}

/// The outputs of the desk the sheets were designed on.
#[derive(Clone, Copy, ValueEnum)]
enum Desk {
    /// 2560 × 1600 at 1.666667.
    Laptop,
    /// 3440 × 1440 at 1.
    Ultrawide,
}

/// Which computer the sheets of this computer draw.
#[derive(Clone, Copy, ValueEnum)]
enum Source {
    /// This one.
    Live,
    /// A made-up laptop, the same every time: what committed images show.
    Fixture,
    /// A made-up desktop tower: a graphics card with two displays, case
    /// fans, three drives, no battery.
    FixtureDesktop,
    /// A made-up two-socket server: many cores and drives, no display.
    FixtureServer,
    /// A made-up virtual machine, with almost nothing.
    FixtureVm,
}

/// A subject's index from its name.
fn subject(subjects: &[Box<dyn Subject>], name: &str) -> Result<usize, String> {
    subjects::find(subjects, name).ok_or_else(|| {
        let names: Vec<_> = subjects.iter().map(|s| s.name()).collect();
        format!("no subject {name:?} (have: {})", names.join(", "))
    })
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();
    let machine = match cli.machine {
        Source::Live => Machine::live(),
        Source::Fixture => Machine::fixture(),
        Source::FixtureDesktop => Fixture::Desktop.machine(),
        Source::FixtureServer => Fixture::Server.machine(),
        Source::FixtureVm => Fixture::Vm.machine(),
    };
    let result = match cli.command {
        Some(Command::Render(render)) => draw(render, &machine),
        Some(Command::Bench(bench)) => time(bench, &machine),
        Some(Command::List) => {
            for subject in subjects::all(&machine) {
                let card = subject.card();
                println!(
                    "{:<10} {:<28} {}",
                    subject.name(),
                    card.title,
                    card.domain.label()
                );
            }
            Ok(())
        }
        None => run(cli.run, machine),
    };

    if let Err(error) = result {
        eprintln!("quadrille-screensaver: {error}");
        std::process::exit(1);
    }
}

fn run(run: Run, machine: Machine) -> Result<(), String> {
    let first = match &run.subject {
        Some(name) => Some(subject(&subjects::all(&machine), name)?),
        None => None,
    };

    // One screensaver at a time: a second one started while the first runs
    // (by the idle service and a key, say) leaves quietly.
    let _lock = match single_instance(&run.namespace) {
        Ok(lock) => lock,
        Err(error) => {
            log::info!("{error}");
            return Ok(());
        }
    };

    let options = Options {
        namespace: run.namespace,
        first,
        seed: run.seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos() as u64)
        }),
        fps: run.fps,
        grace: Duration::from_millis(run.grace),
        theme: theme::current(&run.theme_dir),
        date: today(),
        machine,
    };

    let mut settings = quadrille_desktop::graphics::settings();
    settings.backend = Backend::Custom(run.backend);
    quadrille_desktop::graphics::fall_back_to_departure();

    let boot = std::cell::RefCell::new(Some(options));

    let result = iced_layer::application(
        move || Saver::new(boot.borrow_mut().take().expect("Boot once")),
        Saver::update,
        |saver: &Saver, window| saver.view(window),
        |saver: &Saver, env| saver.surfaces(env),
    )
    .settings(settings)
    .subscription(Saver::subscription)
    .theme(|saver: &Saver, _| saver.theme())
    .run();

    result.map_err(|error| error.to_string())
}

fn draw(render: Render, machine: &Machine) -> Result<(), String> {
    let (output, desk) = match render.output {
        Desk::Laptop => (Output::LAPTOP, "laptop"),
        Desk::Ultrawide => (Output::ULTRAWIDE, "ultrawide"),
    };
    let (output, desk) = match render.size {
        Some(size) => (size, format!("{}x{}", size.width, size.height)),
        None => (output, desk.to_owned()),
    };
    let theme = match &render.theme {
        Some(name) => theme::quadrille_theme(&format!("quadrille-{name}"))
            .ok_or(format!("no theme {name:?}"))?,
        None => theme::current(&theme::default_dir()),
    };
    let tag = render.theme.as_deref().unwrap_or("current");

    std::fs::create_dir_all(&render.out)
        .map_err(|error| format!("{}: {error}", render.out.display()))?;
    headless::load_fonts();

    let subjects = subjects::all(machine);
    let indices: Vec<usize> = match &render.subject {
        Some(name) => vec![subject(&subjects, name)?],
        None => (0..subjects.len()).collect(),
    };
    let date = today();
    let mut studio = Studio::new(&subjects, output, &theme, &date)?;

    for index in indices {
        for &moment in &render.at {
            let path = render.out.join(format!(
                "{}-{desk}-{tag}-{moment:05.1}.png",
                subjects[index].name()
            ));

            studio.save(index, moment, render.physical, &path)?;
            println!("{}", path.display());
        }
    }

    Ok(())
}

fn time(bench: Bench, machine: &Machine) -> Result<(), String> {
    let output = match bench.output {
        Desk::Laptop => Output::LAPTOP,
        Desk::Ultrawide => Output::ULTRAWIDE,
    };
    let theme = theme::current(&theme::default_dir());
    let subjects = subjects::all(machine);
    let date = today();

    headless::load_fonts();
    let ms = |duration: Duration| duration.as_secs_f64() * 1e3;
    let stages = [Stage::Plot, Stage::DetailIn, Stage::Settled, Stage::Wipe];

    println!(
        "a frame on the {}, 95th percentile / worst: milliseconds drawing the sheet, milliseconds repainting what changed, and the share of the output that is",
        match bench.output {
            Desk::Laptop => "laptop",
            Desk::Ultrawide => "ultrawide",
        }
    );
    print!("{:<10}", "");
    for stage in stages {
        print!(" {:<34}", stage.name());
    }
    println!();

    for (index, subject) in subjects.iter().enumerate() {
        // From a fresh studio: the first frame repaints everything.
        let mut studio = Studio::new(&subjects, output, &theme, &date)?;
        let frames = studio.bench(index, bench.fps);

        print!("{:<10}", subject.name());

        for stage in stages {
            let of_stage = || frames.iter().skip(1).filter(|f| f.stage == stage);
            let mut draw: Vec<f64> = of_stage().map(|f| ms(f.draw)).collect();
            let mut repaint: Vec<f64> = of_stage().map(|f| ms(f.repaint.took)).collect();
            let mut share: Vec<f64> = of_stage()
                .map(|f| f64::from(f.repaint.share) * 100.0)
                .collect();

            let spread = |values: &mut Vec<f64>| {
                values.sort_by(f64::total_cmp);
                values
                    .last()
                    .map(|worst| (values[values.len() * 95 / 100], *worst))
            };

            let cell = match (spread(&mut draw), spread(&mut repaint), spread(&mut share)) {
                (Some(draw), Some(repaint), Some(share)) => format!(
                    "{:4.1} {:4.1}  {:4.1} {:4.1}  {:3.0}% {:3.0}%",
                    draw.0, draw.1, repaint.0, repaint.1, share.0, share.1
                ),
                _ => "-".into(),
            };

            print!(" {cell:<34}");
        }

        println!();
    }

    Ok(())
}

/// Holds `$XDG_RUNTIME_DIR/NAMESPACE.lock` for as long as the returned file
/// is open: one screensaver per namespace, so a test never meets the real
/// one.
fn single_instance(namespace: &str) -> Result<std::fs::File, String> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let name: String = namespace
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(format!("{name}.lock"));
    let file =
        std::fs::File::create(&path).map_err(|error| format!("{}: {error}", path.display()))?;

    file.try_lock()
        .map_err(|_| "a screensaver is already running".to_owned())?;

    Ok(file)
}

/// Today's date in local time, as a title block writes it.
fn today() -> String {
    jiff::Zoned::now().strftime("%Y-%m-%d").to_string()
}
