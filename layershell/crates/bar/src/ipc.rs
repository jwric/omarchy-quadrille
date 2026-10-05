//! A socket to summon the panels by: what a shell script, or the QML side of
//! Omarchy's shell, runs as `quadrille-bar ctl toggle-popup`.
use iced_futures::futures::SinkExt;
use iced_futures::futures::channel::mpsc;
use iced_futures::stream;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    runtime.join("quadrille-bar.sock")
}

/// Sends one command to the running bar.
pub fn send(command: &str) -> std::io::Result<()> {
    let mut stream = UnixStream::connect(socket_path())?;

    stream.write_all(command.as_bytes())?;
    stream.write_all(b"\n")
}

/// Whether a bar is already listening.
pub fn is_running() -> bool {
    UnixStream::connect(socket_path()).is_ok()
}

/// The commands that arrive on the socket, a line each.
pub fn commands() -> impl iced_futures::futures::Stream<Item = String> {
    stream::channel(16, async |mut output: mpsc::Sender<String>| {
        let path = socket_path();
        let _ = std::fs::remove_file(&path);

        let Ok(listener) = UnixListener::bind(&path) else {
            log::warn!("could not listen on {}", path.display());

            return std::future::pending::<()>().await;
        };

        let (sender, mut receiver) = mpsc::unbounded::<String>();

        let _ = std::thread::Builder::new()
            .name("ipc".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    for line in BufReader::new(stream).lines().map_while(Result::ok) {
                        let _ = sender.unbounded_send(line);
                    }
                }
            });

        use iced_futures::futures::StreamExt;

        while let Some(line) = receiver.next().await {
            let _ = output.send(line).await;
        }
    })
}
