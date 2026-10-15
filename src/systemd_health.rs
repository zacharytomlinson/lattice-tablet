//! Local appliance readiness reporting for systemd.
//!
//! Readiness is tied to a rendered frame and watchdog traffic is emitted from
//! Slint's event loop. Network connectivity and Lattice availability are not
//! part of this health boundary.

use std::env;
use std::ffi::OsStr;
use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::time::Duration;

use slint::{ComponentHandle, RenderingState, Timer, TimerMode};

use crate::MainWindow;

const READY_MESSAGE: &[u8] = b"READY=1\nSTATUS=Lattice tablet rendered its first frame";
const WATCHDOG_MESSAGE: &[u8] = b"WATCHDOG=1";

#[derive(Debug)]
struct SystemdNotifier {
    socket: UnixDatagram,
    target: SocketAddr,
}

impl SystemdNotifier {
    fn from_environment() -> io::Result<Option<Self>> {
        let Some(name) = env::var_os("NOTIFY_SOCKET") else {
            return Ok(None);
        };
        Self::from_socket_name(&name).map(Some)
    }

    fn from_socket_name(name: &OsStr) -> io::Result<Self> {
        let target = match name.as_bytes() {
            [b'@', abstract_name @ ..] => SocketAddr::from_abstract_name(abstract_name)?,
            _ => SocketAddr::from_pathname(name)?,
        };
        Ok(Self {
            socket: UnixDatagram::unbound()?,
            target,
        })
    }

    fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            socket: self.socket.try_clone()?,
            target: self.target.clone(),
        })
    }

    fn send(&self, message: &[u8]) -> io::Result<()> {
        let written = self.socket.send_to_addr(message, &self.target)?;
        if written == message.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "systemd notification was truncated",
            ))
        }
    }
}

fn watchdog_interval() -> Option<Duration> {
    if env::var("WATCHDOG_PID")
        .ok()
        .and_then(|pid| pid.parse::<u32>().ok())
        .is_some_and(|pid| pid != std::process::id())
    {
        return None;
    }

    let usec = env::var("WATCHDOG_USEC").ok()?.parse::<u64>().ok()?;
    (usec > 0).then(|| Duration::from_micros((usec / 2).max(1_000)))
}

/// Installs first-frame readiness and an event-loop watchdog when systemd
/// supplied a notification socket. The returned timer must remain alive for
/// the duration of the Slint event loop.
pub fn install(window: &MainWindow) -> Result<Option<Timer>, Box<dyn std::error::Error>> {
    let Some(notifier) = SystemdNotifier::from_environment()? else {
        return Ok(None);
    };

    let ready_notifier = notifier.try_clone()?;
    let mut ready_sent = false;
    window
        .window()
        .set_rendering_notifier(move |state, _graphics_api| {
            if !ready_sent && matches!(state, RenderingState::AfterRendering) {
                match ready_notifier.send(READY_MESSAGE) {
                    Ok(()) => ready_sent = true,
                    Err(error) => eprintln!("failed to report first-frame readiness: {error}"),
                }
            }
        })?;

    let Some(interval) = watchdog_interval() else {
        return Ok(None);
    };
    let watchdog = Timer::default();
    watchdog.start(TimerMode::Repeated, interval, move || {
        if let Err(error) = notifier.send(WATCHDOG_MESSAGE) {
            eprintln!("failed to notify systemd watchdog: {error}");
        }
    });
    Ok(Some(watchdog))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixDatagram};
    use std::time::{Duration, SystemTime};

    use super::SystemdNotifier;

    #[test]
    fn filesystem_notification_reaches_the_bound_socket() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock follows Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "lattice-tablet-notify-{}-{unique}",
            std::process::id()
        ));
        let receiver = UnixDatagram::bind(&path).expect("bind notification test socket");
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set notification test timeout");

        let notifier = SystemdNotifier::from_socket_name(path.as_os_str())
            .expect("construct filesystem notifier");
        notifier.send(b"READY=1").expect("send notification");

        let mut bytes = [0_u8; 32];
        let count = receiver.recv(&mut bytes).expect("receive notification");
        assert_eq!(&bytes[..count], b"READY=1");
        fs::remove_file(path).expect("remove notification test socket");
    }

    #[test]
    fn abstract_notification_reaches_the_bound_socket() {
        let name = format!("lattice-tablet-notify-{}", std::process::id());
        let address = SocketAddr::from_abstract_name(name.as_bytes())
            .expect("construct abstract notification address");
        let receiver =
            UnixDatagram::bind_addr(&address).expect("bind abstract notification socket");
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set notification test timeout");

        let socket_name = format!("@{name}");
        let notifier = SystemdNotifier::from_socket_name(socket_name.as_ref())
            .expect("construct abstract notifier");
        notifier.send(b"WATCHDOG=1").expect("send notification");

        let mut bytes = [0_u8; 32];
        let count = receiver.recv(&mut bytes).expect("receive notification");
        assert_eq!(&bytes[..count], b"WATCHDOG=1");
    }
}
