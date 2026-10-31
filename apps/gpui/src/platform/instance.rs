//! One copy of the app at a time: a second start hands its `wonnet://` link (or only the wish to
//! be shown) to the running one over a local socket, and quits.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, ListenerOptions, Name, Stream};

fn name(dir: &Path) -> std::io::Result<Name<'static>> {
    if cfg!(windows) {
        return "wonnet-instance".to_ns_name::<GenericNamespaced>().map(|name| name.into_owned());
    }
    dir.join("instance.sock").to_fs_name::<GenericFilePath>().map(|name| name.into_owned())
}

/// Gives the links to a running copy. True when there is one, and this copy should quit.
pub fn hand_over(dir: &Path, links: &[String]) -> bool {
    let Ok(name) = name(dir) else { return false };
    let Ok(mut stream) = Stream::connect(name) else { return false };
    let message =
        if links.is_empty() { "show\n".to_string() } else { links.iter().map(|link| format!("{link}\n")).collect() };
    stream.write_all(message.as_bytes()).is_ok()
}

/// Waits, a few seconds at most, for the copy this one replaces to quit: an update starts the
/// new version before the old one has let go.
pub fn wait_for_previous(dir: &Path) {
    for _ in 0..50 {
        let Ok(name) = name(dir) else { return };
        if Stream::connect(name).is_err() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Listens for later starts; each line they send goes to `heard`: a link, or `show`.
pub fn listen(dir: &Path, heard: async_channel::Sender<String>) {
    if !cfg!(windows) {
        let _ = std::fs::remove_file(dir.join("instance.sock"));
    }
    let Ok(name) = name(dir) else { return };
    let listener = match ListenerOptions::new().name(name).create_sync() {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Another start can't reach this one: {error}");
            return;
        }
    };
    std::thread::spawn(move || {
        for connection in listener.incoming().flatten() {
            for line in BufReader::new(connection).lines().map_while(Result::ok) {
                let _ = heard.send_blocking(line);
            }
        }
    });
}
