//! The control socket: what a Hyprland binding or an Omarchy menu action runs
//! as `quadrille-bar ctl toggle sysmon`.
//!
//! One command per connection, a line of text; the answer comes back as text
//! before the connection closes, and a line that starts with `error:` makes
//! `ctl` exit with 1.
use iced_futures::futures::SinkExt;
use iced_futures::futures::StreamExt;
use iced_futures::futures::channel::mpsc;
use iced_futures::stream;

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn socket_path() -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    let name =
        std::env::var("QUADRILLE_BAR_SOCKET").unwrap_or_else(|_| "quadrille-bar.sock".into());

    runtime.join(name)
}

/// Sends one command to the running bar and returns its answer.
pub fn send(command: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket_path())?;

    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(command.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.shutdown(std::net::Shutdown::Write)?;

    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer)?;

    Ok(answer)
}

/// Whether a bar is already listening.
pub fn is_running() -> bool {
    UnixStream::connect(socket_path()).is_ok()
}

/// A command waiting for its answer.
#[derive(Clone)]
pub struct Request {
    pub line: String,
    reply: Arc<Mutex<Option<std_mpsc::Sender<(String, std_mpsc::Sender<()>)>>>>,
}

impl Request {
    /// Answers the command. Only the first answer is sent.
    pub fn respond(&self, answer: impl Into<String>) {
        if let Some(reply) = self.reply.lock().ok().and_then(|mut reply| reply.take()) {
            let (written, receipt) = std_mpsc::channel();
            if reply.send((answer.into(), written)).is_ok() {
                // A quit may end the process immediately after this returns.
                // Give the client thread time to put its answer on the socket.
                let _ = receipt.recv_timeout(Duration::from_millis(100));
            }
        }
    }
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Request({:?})", self.line)
    }
}

/// The commands that arrive on the socket.
pub fn requests() -> impl iced_futures::futures::Stream<Item = Request> {
    stream::channel(16, async |mut output: mpsc::Sender<Request>| {
        let path = socket_path();
        let _ = std::fs::remove_file(&path);

        let Ok(listener) = UnixListener::bind(&path) else {
            log::warn!("could not listen on {}", path.display());

            return std::future::pending::<()>().await;
        };

        let (sender, mut receiver) = mpsc::unbounded::<Request>();

        let _ = std::thread::Builder::new()
            .name("ipc".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let sender = sender.clone();

                    // One thread per connection, so that a client that never
                    // finishes its line holds nobody up.
                    let _ = std::thread::Builder::new()
                        .name("ipc-client".into())
                        .spawn(move || serve(stream, &sender));
                }
            });

        while let Some(request) = receiver.next().await {
            let _ = output.send(request).await;
        }
    })
}

fn serve(mut stream: UnixStream, sender: &mpsc::UnboundedSender<Request>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

    let mut line = String::new();

    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }

    let line = line.trim().to_owned();

    if line.is_empty() {
        return;
    }

    let (reply, answer) = std_mpsc::channel();

    let request = Request {
        line,
        reply: Arc::new(Mutex::new(Some(reply))),
    };

    if sender.unbounded_send(request).is_err() {
        return;
    }

    let (answer, receipt) = answer
        .recv_timeout(Duration::from_secs(3))
        .unwrap_or_else(|_| ("error: no answer\n".into(), std_mpsc::channel().0));

    let _ = stream.write_all(answer.as_bytes());
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = receipt.send(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_reply_reaches_the_socket_before_respond_returns() {
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let (sender, mut requests) = mpsc::unbounded();
        let worker = std::thread::spawn(move || serve(server, &sender));
        client.write_all(b"quit\n").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let request = smol::block_on(requests.next()).unwrap();
        request.respond("bye\n");
        request.respond("another reply\n");
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert_eq!(reply, "bye\n");
        worker.join().unwrap();
    }
}
