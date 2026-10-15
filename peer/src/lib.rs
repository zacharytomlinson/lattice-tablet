//! Linux transport for the tablet's attached peer. No UI or household model.
use lattice_host_link::{Decoder, MAX_PACKET};
pub use lattice_host_link::{Failure, Packet};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::{Duration, Instant},
};

pub struct Client {
    file: File,
    decoder: Decoder,
    token: u32,
}
impl Client {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(path)?;
        let fd = file.as_raw_fd();
        // Kernel exclusion prevents another application from interleaving this
        // binary stream. The descriptor is closed if any configuration step fails.
        if unsafe { libc::ioctl(fd, libc::TIOCEXCL) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut termios) } < 0 {
            return Err(io::Error::last_os_error());
        }
        unsafe {
            libc::cfmakeraw(&mut termios);
            libc::cfsetispeed(&mut termios, libc::B115200);
            libc::cfsetospeed(&mut termios, libc::B115200);
        }
        termios.c_cflag |= libc::CLOCAL | libc::CREAD | libc::HUPCL;
        termios.c_cflag &= !libc::CRTSCTS;
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } < 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::tcflush(fd, libc::TCIOFLUSH) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let dtr = libc::TIOCM_DTR;
        if unsafe { libc::ioctl(fd, libc::TIOCMBIS, &dtr) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            file,
            decoder: Decoder::new(),
            token: 0,
        })
    }
    pub fn status(&mut self) -> io::Result<Packet> {
        self.token = self.token.wrapping_add(1);
        self.exchange(Packet::Status { token: self.token })
    }
    pub fn read_links(&mut self, owner: u64, after: u64, via: u64) -> io::Result<Packet> {
        self.token = self.token.wrapping_add(1);
        self.exchange(Packet::ReadLinks {
            token: self.token,
            owner,
            after,
            via,
        })
    }
    fn exchange(&mut self, request: Packet) -> io::Result<Packet> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut encoded = [0; MAX_PACKET];
        let n = request.encode(&mut encoded);
        // Leading delimiter discards any interrupted prior record after reconnect.
        let mut output = [0; MAX_PACKET + 1];
        output[1..n + 1].copy_from_slice(&encoded[..n]);
        let mut sent = 0;
        while sent < n + 1 {
            ready(&self.file, libc::POLLOUT, deadline)?;
            match self.file.write(&output[sent..n + 1]) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => sent += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(e),
            }
        }
        let mut input = [0; 256];
        loop {
            ready(&self.file, libc::POLLIN, deadline)?;
            let n = match self.file.read(&mut input) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => n,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(e) => return Err(e),
            };
            for byte in &input[..n] {
                if let Some(reply) = self.decoder.push(*byte) {
                    if reply.token() != request.token() {
                        continue;
                    }
                    let matches = match (request, reply) {
                        (_, Packet::Error { .. }) => true,
                        (Packet::Status { .. }, Packet::Ready { .. }) => true,
                        (
                            Packet::ReadLinks {
                                owner, after, via, ..
                            },
                            Packet::Link {
                                owner: o,
                                after: a,
                                via: v,
                                ..
                            },
                        ) => (owner, after, via) == (o, a, v),
                        _ => false,
                    };
                    if matches {
                        return Ok(reply);
                    }
                }
            }
        }
    }
}
fn ready(file: &File, events: i16, deadline: Instant) -> io::Result<()> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "host link timed out; request was not retried",
            ));
        }
        let mut poll = libc::pollfd {
            fd: file.as_raw_fd(),
            events,
            revents: 0,
        };
        let result = unsafe {
            libc::poll(
                &mut poll,
                1,
                remaining.as_millis().min(i32::MAX as u128) as i32,
            )
        };
        if result < 0 {
            let e = io::Error::last_os_error();
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        if poll.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "attached radio disconnected",
            ));
        }
        if poll.revents & events != 0 {
            return Ok(());
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let dtr = libc::TIOCM_DTR;
        unsafe {
            libc::ioctl(self.file.as_raw_fd(), libc::TIOCMBIC, &dtr);
            libc::ioctl(self.file.as_raw_fd(), libc::TIOCNXCL);
        }
    }
}
