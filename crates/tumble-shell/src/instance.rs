//! Batching right-click invocations into one job (PRD section 11).
//!
//! Explorer starts one `tumblew.exe` per selected file. The first process
//! for a given key (the target format) becomes the leader: it holds a named
//! mutex and listens on a named pipe. Later processes find the mutex, send
//! their paths through the pipe and exit. The leader stops listening once no
//! new paths have arrived for `window`, releases the mutex, and converts the
//! whole batch. A process that arrives after that simply leads a new batch.
//!
//! Names include the Windows session, so other signed-in users and remote
//! sessions never join each other's batches; the pipe rejects remote
//! clients.

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tumble_core::brand;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, GENERIC_WRITE, GetLastError, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_NONE, OPEN_EXISTING, PIPE_ACCESS_INBOUND,
    ReadFile, WriteFile,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows::core::HSTRING;

/// How long a client keeps trying to hand its paths over before leading a
/// batch of its own.
const JOIN_DEADLINE: Duration = Duration::from_secs(10);

pub enum Role {
    /// This process converts these paths (its own and everyone else's).
    Leader(Vec<PathBuf>),
    /// Another process took the paths; this one should exit quietly.
    Joined,
}

struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: a handle we opened and close once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn names(key: &str) -> (String, String) {
    let mut session = 0u32;
    // SAFETY: out-pointer is a valid local.
    let _ = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) };
    let key: String =
        key.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let base = format!("{}-{session}-{key}", brand::APP_NAME);
    (format!(r"Local\{base}"), format!(r"\\.\pipe\{base}"))
}

/// Takes the leader's mutex if nobody holds it.
fn try_lead(mutex_name: &str) -> Option<Owned> {
    // SAFETY: plain object creation; the handle is owned by the result.
    let handle = unsafe { CreateMutexW(None, false, &HSTRING::from(mutex_name)) }.ok()?;
    let handle = Owned(handle);
    // SAFETY: reads the error from the call just made.
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS { None } else { Some(handle) }
}

fn encode(paths: &[PathBuf]) -> Vec<u8> {
    let mut units: Vec<u16> = Vec::new();
    for p in paths {
        units.extend(p.as_os_str().encode_wide());
        units.push(0);
    }
    units.iter().flat_map(|u| u.to_le_bytes()).collect()
}

fn decode(bytes: &[u8]) -> Vec<PathBuf> {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect();
    units
        .split(|&u| u == 0)
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(OsString::from_wide(s)))
        .collect()
}

fn send(pipe_name: &str, paths: &[PathBuf]) -> io::Result<()> {
    // SAFETY: opening an existing pipe for writing.
    let handle = unsafe {
        CreateFileW(
            &HSTRING::from(pipe_name),
            GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(io::Error::other)?;
    let handle = Owned(handle);
    let data = encode(paths);
    let mut sent = 0usize;
    while sent < data.len() {
        let mut n = 0u32;
        // SAFETY: writes from a live buffer.
        unsafe { WriteFile(handle.0, Some(&data[sent..]), Some(&mut n), None) }
            .map_err(io::Error::other)?;
        sent += n as usize;
    }
    Ok(())
}

/// Accepts clients until `stop`, forwarding each one's paths.
fn serve(pipe_name: String, stop: Arc<AtomicBool>, tx: mpsc::Sender<Vec<PathBuf>>) {
    let name = HSTRING::from(pipe_name);
    loop {
        // SAFETY: creates one pipe instance; owned below.
        let pipe = unsafe {
            CreateNamedPipeW(
                &name,
                PIPE_ACCESS_INBOUND,
                PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                0,
                64 * 1024,
                0,
                None,
            )
        };
        if pipe.is_invalid() {
            return;
        }
        let pipe = Owned(pipe);
        // SAFETY: blocking wait for a client on our instance.
        let connected = unsafe { ConnectNamedPipe(pipe.0, None) };
        if connected.is_err() && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            continue;
        }
        let mut data = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let mut n = 0u32;
            // SAFETY: reads into a live buffer.
            let r = unsafe { ReadFile(pipe.0, Some(&mut buf), Some(&mut n), None) };
            if n > 0 {
                data.extend_from_slice(&buf[..n as usize]);
            }
            // The client closing its end (a broken pipe) ends the message.
            if r.is_err() || n == 0 {
                break;
            }
        }
        // SAFETY: the instance is ours.
        unsafe {
            let _ = DisconnectNamedPipe(pipe.0);
        }
        let paths = decode(&data);
        if !paths.is_empty() {
            let _ = tx.send(paths);
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
    }
}

/// Either leads a batch (collecting everyone's paths until `window` passes
/// with no arrivals) or hands `paths` to the current leader.
pub fn gather(key: &str, paths: Vec<PathBuf>, window: Duration) -> io::Result<Role> {
    let (mutex_name, pipe_name) = names(key);
    let start = Instant::now();
    loop {
        if let Some(mutex) = try_lead(&mutex_name) {
            return Ok(Role::Leader(lead(&pipe_name, mutex, paths, window)));
        }
        if send(&pipe_name, &paths).is_ok() {
            return Ok(Role::Joined);
        }
        if start.elapsed() > JOIN_DEADLINE {
            // Something is wrong with the leader; convert our own files.
            return Ok(Role::Leader(paths));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn lead(pipe_name: &str, mutex: Owned, mut paths: Vec<PathBuf>, window: Duration) -> Vec<PathBuf> {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let server = {
        let (name, stop) = (pipe_name.to_string(), stop.clone());
        std::thread::spawn(move || serve(name, stop, tx))
    };
    while let Ok(more) = rx.recv_timeout(window) {
        paths.extend(more);
    }
    // Stop listening: set the flag, then wake the blocked ConnectNamedPipe
    // with empty connections of our own until the server has noticed (one
    // may land in the gap between two pipe instances).
    stop.store(true, Ordering::SeqCst);
    while !server.is_finished() {
        let _ = send(pipe_name, &[]);
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = server.join();
    // Anything that arrived while stopping still belongs to this batch.
    paths.extend(rx.try_iter().flatten());
    drop(mutex);
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trips_unicode_and_long_paths() {
        let long = PathBuf::from(format!(r"\\?\C:\{}\x.png", "d".repeat(300)));
        let paths = vec![PathBuf::from(r"C:\a b\ф 写.jpg"), long];
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
        let mut all = leaders.pop().unwrap();
        all.sort();
        assert_eq!(all.len(), 12);
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
