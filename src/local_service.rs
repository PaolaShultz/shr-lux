//! Explicit synthetic local binding. Ordinary simulator startup never calls this module.
use crate::wire::Service;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const FRAME_LIMIT: usize = 65536;
const CLIENT_LIMIT: usize = 4;
const FRAME_TIME: Duration = Duration::from_millis(500);
const IDLE_TIME: Duration = Duration::from_secs(3);
const LIFETIME: Duration = Duration::from_secs(30);
/// Verifies an existing private directory; does not create or trust a client pathname.
pub fn private_directory(path: &Path) -> io::Result<()> {
    let m = fs::symlink_metadata(path)?;
    // SAFETY: geteuid has no pointer arguments or side effects.
    let uid = unsafe { libc::geteuid() };
    if !m.is_dir() || m.uid() != uid || m.mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "owned 0700 directory required",
        ));
    }
    if fs::canonicalize(path)? != path {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "canonical directory required",
        ));
    }
    Ok(())
}
pub(crate) fn same_uid(stream: &UnixStream) -> io::Result<()> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: valid fd, writable ucred storage and its correct length.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: geteuid has no pointer arguments.
    if len as usize != std::mem::size_of::<libc::ucred>() || cred.uid != unsafe { libc::geteuid() }
    {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "peer uid"));
    }
    Ok(())
}
fn read_deadline(stream: &mut UnixStream, bytes: &mut [u8], deadline: Instant) -> io::Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "frame deadline"))?;
        stream.set_read_timeout(Some(remaining))?;
        let n = stream.read(&mut bytes[offset..])?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "frame eof"));
        }
        offset += n;
    }
    Ok(())
}
pub fn read_frame(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut header = [0; 4];
    // Idle wait exceeds healthy renewal500ms; frame deadline begins first byte.
    read_deadline(stream, &mut header[..1], Instant::now() + IDLE_TIME)?;
    let deadline = Instant::now() + FRAME_TIME;
    read_deadline(stream, &mut header[1..], deadline)?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > FRAME_LIMIT {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame capacity"));
    }
    let mut bytes = vec![0; size];
    read_deadline(stream, &mut bytes, deadline)?;
    Ok(bytes)
}
pub fn write_frame(stream: &mut UnixStream, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > FRAME_LIMIT {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame capacity"));
    }
    stream.set_write_timeout(Some(FRAME_TIME))?;
    let deadline = Instant::now() + FRAME_TIME;
    let header = (bytes.len() as u32).to_be_bytes();
    for part in [&header[..], bytes] {
        let mut offset = 0;
        while offset < part.len() {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "reader stalled"))?;
            stream.set_write_timeout(Some(remaining))?;
            let n = stream.write(&part[offset..])?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::WriteZero, "reader closed"));
            }
            offset += n;
        }
    }
    Ok(())
}
/// Owns only its created socket and joins every own thread on shutdown.
pub struct LocalServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
    socket: PathBuf,
    identity: (u64, u64),
}
impl LocalServer {
    pub fn bind(directory: &Path, service: Service) -> io::Result<Self> {
        Self::bind_with_analysis(directory, service, None)
    }
    pub fn bind_with_analysis(
        directory: &Path,
        service: Service,
        analysis: Option<crate::analysis_subscription::Client>,
    ) -> io::Result<Self> {
        private_directory(directory)?;
        let socket = directory.join("lux.sock");
        match fs::symlink_metadata(&socket) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "endpoint exists",
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let listener = UnixListener::bind(&socket)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        let metadata = fs::symlink_metadata(&socket)?;
        let identity = (metadata.dev(), metadata.ino());
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let state = Arc::new(Mutex::new(service));
        let thread = thread::spawn(move || {
            let start = Instant::now();
            let mut clients: Vec<JoinHandle<()>> = Vec::new();
            while !worker_stop.load(Ordering::Acquire) {
                {
                    let mut service = state
                        .lock()
                        .map_err(|_| io::Error::other("authority poison"))?;
                    let tick = u64::try_from(start.elapsed().as_millis() / 10)
                        .map_err(io::Error::other)?;
                    let now = crate::analysis_subscription::monotonic_ms()?;
                    service
                        .poll_analysis(analysis.as_ref(), now, tick)
                        .map_err(io::Error::other)?;
                }
                let mut i = 0;
                while i < clients.len() {
                    if clients[i].is_finished() {
                        let child = clients.swap_remove(i);
                        let _ = child.join();
                    } else {
                        i += 1;
                    }
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        if clients.len() >= CLIENT_LIMIT || same_uid(&stream).is_err() {
                            continue;
                        }
                        let state = state.clone();
                        let stop = worker_stop.clone();
                        clients.push(thread::spawn(move || {
                            let connected = Instant::now();
                            let mut work = 0;
                            while !stop.load(Ordering::Acquire)
                                && connected.elapsed() < LIFETIME
                                && work < 1024
                            {
                                let Ok(bytes) = read_frame(&mut stream) else {
                                    break;
                                };
                                work += 1;
                                let responses = {
                                    let Ok(mut service) = state.lock() else {
                                        break;
                                    };
                                    let ticks = start.elapsed().as_millis() / 10;
                                    let Ok(tick) = u64::try_from(ticks) else {
                                        break;
                                    };
                                    let Ok(now) = crate::analysis_subscription::monotonic_ms()
                                    else {
                                        break;
                                    };
                                    if service.poll_analysis(None, now, tick).is_err() {
                                        break;
                                    }
                                    service.handle(&bytes)
                                };
                                // Synchronous request/reply: no telemetry subscription and <=16 queued pages.
                                if responses.len() > 32 {
                                    break;
                                }
                                let mut failed = false;
                                for response in responses {
                                    let Ok(bytes) = serde_json::to_vec(&response) else {
                                        failed = true;
                                        break;
                                    };
                                    if write_frame(&mut stream, &bytes).is_err() {
                                        failed = true;
                                        break;
                                    }
                                }
                                if failed {
                                    break;
                                }
                            }
                        }));
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => {
                        for c in clients {
                            let _ = c.join();
                        }
                        return Err(e);
                    }
                }
            }
            for c in clients {
                let _ = c.join();
            }
            Ok(())
        });
        Ok(Self {
            stop,
            thread: Some(thread),
            socket,
            identity,
        })
    }
    pub fn socket(&self) -> &Path {
        &self.socket
    }
    pub fn shutdown(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        let result = if let Some(t) = self.thread.take() {
            t.join()
                .map_err(|_| io::Error::other("service thread panic"))?
        } else {
            Ok(())
        };
        if let Ok(m) = fs::symlink_metadata(&self.socket)
            && m.file_type().is_socket()
            && (m.dev(), m.ino()) == self.identity
        {
            fs::remove_file(&self.socket)?;
        }
        result
    }
}
impl Drop for LocalServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
