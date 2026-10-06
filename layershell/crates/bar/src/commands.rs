//! Running the programs the panels read the system through, and act on it with.
//!
//! The panels never start a process themselves: they hold a [`Runner`], which
//! is the real one ([`System`]) in normal use. With `QUADRILLE_COMMANDS` set
//! to a directory it is [`Stubs`] instead, which runs `DIR/<program>` and
//! nothing else: a command with no stub there fails, it is never looked up on
//! the `PATH`. That is how the panels are tested with real clicks and keys
//! without ever changing the machine: every state-changing command (a default
//! sink, a network, a power action) goes to a script that only writes down that
//! it was called.
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

/// How long a command may take unless it says otherwise.
pub const TIMEOUT: Duration = Duration::from_secs(4);

/// How long commands that wait for a device or a scan may take.
pub const LONG: Duration = Duration::from_secs(25);

pub trait Runner: Send + Sync + 'static {
    /// Runs `program` with `args` and gives back what it printed, or what went
    /// wrong: its standard error, or why it could not run.
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Result<String, String>;
}

pub type Shared = Arc<dyn Runner>;

/// What a command asks of the machine, as a list of words: the program first.
pub type Call = Vec<String>;

/// The programs of the machine.
#[derive(Debug, Default)]
pub struct System;

impl Runner for System {
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
        execute(Path::new(program), args, timeout)
    }
}

/// The scripts of a directory, standing in for the programs.
#[derive(Debug)]
pub struct Stubs {
    dir: PathBuf,
}

impl Stubs {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

impl Runner for Stubs {
    fn run(&self, program: &str, args: &[&str], timeout: Duration) -> Result<String, String> {
        let name = Path::new(program)
            .file_name()
            .ok_or_else(|| format!("{program}: not a program"))?;

        let stub = self.dir.join(name);

        if !stub.is_file() {
            return Err(format!(
                "{program}: no stub in {} (stubbed commands run nothing else)",
                self.dir.display()
            ));
        }

        execute(&stub, args, timeout)
    }
}

/// The runner this process uses: the stubs of `QUADRILLE_COMMANDS` if it is
/// set, the system otherwise.
pub fn from_env() -> Shared {
    match std::env::var_os("QUADRILLE_COMMANDS") {
        Some(dir) if !dir.is_empty() => {
            log::warn!("commands are stubbed: only the scripts in {dir:?} run");

            Arc::new(Stubs::new(dir))
        }
        _ => Arc::new(System),
    }
}

fn execute(program: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{}: {error}", program.display()))?;

    let pid = child.id();

    // The output is read while the command runs, so one that prints more than
    // a pipe holds does not wait for us; and a command that hangs is killed.
    let (sender, receiver) = mpsc::channel();

    let _ = std::thread::Builder::new()
        .name("command".into())
        .spawn(move || {
            let _ = sender.send(child.wait_with_output());
        });

    match receiver.recv_timeout(timeout) {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                let error = error.trim();

                Err(if error.is_empty() {
                    format!("{} failed: {}", program.display(), output.status)
                } else {
                    error.lines().next().unwrap_or(error).to_owned()
                })
            }
        }
        Ok(Err(error)) => Err(format!("{}: {error}", program.display())),
        Err(_) => {
            // SAFETY: a signal to a process this function started.
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }

            Err(format!("{} timed out", program.display()))
        }
    }
}

/// Runs `work` where it does not block the interface.
pub async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    smol::unblock(work).await
}

/// A runner that remembers what it was asked and answers from a table: for
/// the tests of what the panels send.
#[cfg(test)]
pub mod recorder {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct Recorder {
        calls: Mutex<Vec<Call>>,
        answers: Mutex<Vec<(String, Result<String, String>)>>,
    }

    impl Recorder {
        pub fn shared() -> (Arc<Recorder>, Shared) {
            let recorder = Arc::new(Recorder::default());
            let shared: Shared = recorder.clone();

            (recorder, shared)
        }

        /// What `program args...` (a prefix of the call) answers from now on.
        pub fn answer(&self, call: &str, answer: Result<&str, &str>) {
            self.answers.lock().unwrap().push((
                call.to_owned(),
                answer.map(str::to_owned).map_err(str::to_owned),
            ));
        }

        /// Every call so far, as `program arg arg`.
        pub fn calls(&self) -> Vec<String> {
            self.calls
                .lock()
                .unwrap()
                .iter()
                .map(|call| call.join(" "))
                .collect()
        }

        pub fn clear(&self) {
            self.calls.lock().unwrap().clear();
        }
    }

    impl Runner for Recorder {
        fn run(&self, program: &str, args: &[&str], _timeout: Duration) -> Result<String, String> {
            let mut call = vec![program.to_owned()];
            call.extend(args.iter().map(|arg| (*arg).to_owned()));

            let line = call.join(" ");
            self.calls.lock().unwrap().push(call);

            self.answers
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|(prefix, _)| line.starts_with(prefix.as_str()))
                .map(|(_, answer)| answer.clone())
                .unwrap_or_else(|| Ok(String::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn a_command_gives_back_what_it_printed() {
        assert_eq!(
            System.run("printf", &["%s", "hello"], TIMEOUT).as_deref(),
            Ok("hello")
        );
    }

    #[test]
    fn a_failing_command_gives_back_its_complaint() {
        let error = System
            .run("sh", &["-c", "echo nope >&2; exit 3"], TIMEOUT)
            .unwrap_err();

        assert_eq!(error, "nope");
    }

    #[test]
    fn a_missing_program_is_an_error() {
        assert!(
            System
                .run("quadrille-no-such-program", &[], TIMEOUT)
                .is_err()
        );
    }

    #[test]
    fn a_hanging_command_is_killed() {
        let started = std::time::Instant::now();
        let error = System
            .run("sleep", &["30"], Duration::from_millis(200))
            .unwrap_err();

        assert!(error.contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_lot_of_output_does_not_stall_a_command() {
        let output = System
            .run(
                "sh",
                &["-c", "head -c 400000 /dev/zero | tr '\\0' x"],
                TIMEOUT,
            )
            .unwrap();

        assert_eq!(output.len(), 400_000);
    }

    #[test]
    fn stubs_run_their_own_scripts_and_nothing_else() {
        let dir = std::env::temp_dir().join(format!("quadrille-stubs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let script = dir.join("hello");
        let mut file = std::fs::File::create(&script).unwrap();
        writeln!(file, "#!/bin/sh\necho \"stub: $@\"").unwrap();
        drop(file);

        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let stubs = Stubs::new(&dir);

        // A script that has only just been written can be "busy" for a moment
        // if another test's fork caught its file descriptor open: that is the
        // test's race, not the runner's, so it is run again.
        let run = |program: &str, args: &[&str]| loop {
            let answer = stubs.run(program, args, TIMEOUT);

            match &answer {
                Err(error) if error.contains("Text file busy") => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => break answer,
            }
        };

        // By name, or by the path it would have had: the same script.
        assert_eq!(run("hello", &["a", "b"]).as_deref(), Ok("stub: a b\n"));
        assert_eq!(run("/usr/bin/hello", &["c"]).as_deref(), Ok("stub: c\n"));

        // `echo` exists on the system; a stub directory without one does not run it.
        let error = stubs.run("echo", &["real"], TIMEOUT).unwrap_err();

        assert!(error.contains("no stub"), "{error}");

        let _ = std::fs::remove_dir_all(dir);
    }
}
