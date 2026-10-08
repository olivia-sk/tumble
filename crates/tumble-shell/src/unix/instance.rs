//! Batching right-click invocations into one job on macOS and Linux, the
//! same way as on Windows (PRD section 11). Finder and the file managers
//! usually pass every selected file to one process already; this catches
//! several quick right-clicks, and file managers that start one process
//! per file.
//!
//! The first process for a given key (the target format) becomes the
//! leader: it holds an `flock` on `<key>.lock` and listens on the Unix
//! socket `<key>.sock`. Later processes fail to take the lock, send their
//! paths through the socket and exit. The leader stops listening once no
//! new paths have arrived for `window`, removes the socket, reads whatever
//! is still queued, releases the lock and converts the whole batch. A
//! process that arrives after that simply leads a new batch.
//!
//! Both files live in a folder only this user can open
//! (`$XDG_RUNTIME_DIR` on Linux, the per-user `$TMPDIR` on macOS), so other
//! users never join each other's batches.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tumble_core::brand;

/// How long a client keeps trying to hand its paths over before leading a
/// batch of its own.
const JOIN_DEADLINE: Duration = Duration::from_secs(10);

pub enum Role {
    /// This process converts these paths (its own and everyone else's).
    Leader(Vec<PathBuf>),
    /// Another process took the paths; this one should exit quietly.
    Joined,
}

/// A private folder for the lock and socket.
fn private_dir() -> io::Result<PathBuf> {
    // SAFETY: plain query.
    let uid = unsafe { libc::getuid() };
    if cfg!(target_os = "linux")
        && let Some(run) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
        && run.is_absolute()
        && run.is_dir()
    {
        return Ok(run);
    }
    let dir = std::env::temp_dir().join(format!("{}-{uid}", brand::CLI_BIN));
    match fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    // Someone else's folder, or one others can write to, is not used.
    let meta = fs::symlink_metadata(&dir)?;
    if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other(format!("{} is not private", dir.display())));
    }
    Ok(dir)
}

/// The lock and socket paths for `key`. Socket paths are limited to about
/// 100 bytes, so the key part is kept short.
fn names(key: &str) -> io::Result<(PathBuf, PathBuf)> {
    let key: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .take(40)
        .collect();
    let dir = private_dir()?;
    let base = format!("{}-{key}", brand::CLI_BIN);
    Ok((dir.join(format!("{base}.lock")), dir.join(format!("{base}.sock"))))
}

/// Takes the leader's lock if nobody holds it. The lock is released when
/// the file is closed.
fn try_lead(lock_path: &Path) -> io::Result<Option<File>> {
    let file = fs::OpenOptions::new().create(true).truncate(false).write(true).open(lock_path)?;
    // SAFETY: a valid descriptor we own for the call.
    let r = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if r == 0 {
        return Ok(Some(file));
    }
    let e = io::Error::last_os_error();
    if e.raw_os_error() == Some(libc::EWOULDBLOCK) { Ok(None) } else { Err(e) }
}

fn encode(paths: &[PathBuf]) -> Vec<u8> {
    let mut data = Vec::new();
    for p in paths {
        data.extend_from_slice(p.as_os_str().as_bytes());
        data.push(0);
    }
    data
}

fn decode(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(OsStr::from_bytes(s)))
        .collect()
}

fn send(socket: &Path, paths: &[PathBuf]) -> io::Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    stream.write_all(&encode(paths))?;
    stream.shutdown(std::net::Shutdown::Write)
}

/// Reads one client's paths.
fn receive(mut stream: UnixStream) -> Vec<PathBuf> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut data = Vec::new();
    let _ = stream.read_to_end(&mut data);
    decode(&data)
}

/// Either leads a batch (collecting everyone's paths until `window` passes
/// with no arrivals) or hands `paths` to the current leader.
pub fn gather(key: &str, paths: Vec<PathBuf>, window: Duration) -> io::Result<Role> {
    let (lock_path, socket) = names(key)?;
    let start = Instant::now();
    loop {
        if let Some(lock) = try_lead(&lock_path)? {
            return Ok(Role::Leader(lead(&socket, lock, paths, window)));
        }
        if send(&socket, &paths).is_ok() {
            return Ok(Role::Joined);
        }
        if start.elapsed() > JOIN_DEADLINE {
            // Something is wrong with the leader; convert our own files.
            return Ok(Role::Leader(paths));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn lead(socket: &Path, lock: File, mut paths: Vec<PathBuf>, window: Duration) -> Vec<PathBuf> {
    // A socket file left by a crashed leader is stale: we hold the lock.
    let _ = fs::remove_file(socket);
    let Ok(listener) = UnixListener::bind(socket) else {
        return paths;
    };
    if listener.set_nonblocking(true).is_err() {
        let _ = fs::remove_file(socket);
        return paths;
    }
    let mut last = Instant::now();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let more = receive(stream);
                if !more.is_empty() {
                    paths.extend(more);
                    last = Instant::now();
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if last.elapsed() >= window {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
    // New clients now fail to connect and wait for the lock; anyone who
    // connected before this still belongs to this batch.
    let _ = fs::remove_file(socket);
    while let Ok((stream, _)) = listener.accept() {
        paths.extend(receive(stream));
    }
    drop(listener);
    drop(lock);
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trips_unicode_and_odd_names() {
        let paths = vec![PathBuf::from("/a b/ф 写.jpg"), PathBuf::from("/x/new\nline.png")];
        assert_eq!(decode(&encode(&paths)), paths);
        assert!(decode(&[]).is_empty());
    }

    #[test]
    fn concurrent_invocations_form_one_batch() {
        let key = format!("test-{}", std::process::id());
        let mut handles = Vec::new();
        for i in 0..12 {
            let key = key.clone();
            handles.push(std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(i * 20));
                gather(
                    &key,
                    vec![PathBuf::from(format!("file{i}.png"))],
                    Duration::from_millis(500),
                )
                .unwrap()
            }));
        }
        let mut leaders = Vec::new();
        for h in handles {
            if let Role::Leader(paths) = h.join().unwrap() {
                leaders.push(paths);
            }
        }
        assert_eq!(leaders.len(), 1, "one batch");
        assert_eq!(leaders.pop().unwrap().len(), 12);
    }

    #[test]
    fn a_late_arrival_leads_a_new_batch() {
        let key = format!("late-{}", std::process::id());
        let first = gather(&key, vec![PathBuf::from("a.png")], Duration::from_millis(100)).unwrap();
        let second =
            gather(&key, vec![PathBuf::from("b.png")], Duration::from_millis(100)).unwrap();
        assert!(matches!(first, Role::Leader(ref p) if p.len() == 1));
        assert!(matches!(second, Role::Leader(ref p) if p.len() == 1));
    }
}
