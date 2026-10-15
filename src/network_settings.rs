use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkEntry {
    pub id: String,
    pub name: String,
    pub security: String,
    pub signal: i32,
    pub connected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkSnapshot {
    pub available: bool,
    pub scanning: bool,
    pub status: String,
    pub networks: Vec<NetworkEntry>,
}

impl Default for NetworkSnapshot {
    fn default() -> Self {
        Self {
            available: false,
            scanning: false,
            status: "Starting wireless service".to_string(),
            networks: Vec::new(),
        }
    }
}

enum NetworkCommand {
    Scan,
    Connect { id: String, passphrase: String },
    Disconnect,
}

pub struct NetworkSettings {
    snapshot: Arc<RwLock<NetworkSnapshot>>,
    commands: Sender<NetworkCommand>,
}

impl NetworkSettings {
    pub fn start() -> Self {
        let snapshot = Arc::new(RwLock::new(NetworkSnapshot::default()));
        let (commands, receiver) = mpsc::channel();
        let worker_snapshot = Arc::clone(&snapshot);

        thread::Builder::new()
            .name("lattice-network-settings".to_string())
            .spawn(move || worker(receiver, worker_snapshot))
            .expect("network settings worker must start");

        Self { snapshot, commands }
    }

    pub fn snapshot(&self) -> NetworkSnapshot {
        self.snapshot.read().map_or_else(
            |_| NetworkSnapshot {
                status: "Wireless state unavailable".to_string(),
                ..NetworkSnapshot::default()
            },
            |snapshot| snapshot.clone(),
        )
    }

    pub fn scan(&self) {
        let _ = self.commands.send(NetworkCommand::Scan);
    }

    pub fn connect(&self, id: String, passphrase: String) {
        let _ = self
            .commands
            .send(NetworkCommand::Connect { id, passphrase });
    }

    pub fn disconnect(&self) {
        let _ = self.commands.send(NetworkCommand::Disconnect);
    }
}

#[cfg(target_os = "linux")]
fn worker(receiver: Receiver<NetworkCommand>, snapshot: Arc<RwLock<NetworkSnapshot>>) {
    linux::run(receiver, snapshot);
}

#[cfg(not(target_os = "linux"))]
fn worker(receiver: Receiver<NetworkCommand>, snapshot: Arc<RwLock<NetworkSnapshot>>) {
    if let Ok(mut state) = snapshot.write() {
        state.status = "Wireless settings are available on LatticeOS".to_string();
    }
    while receiver.recv().is_ok() {}
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use zbus::blocking::{
        Connection, Proxy, connection::Builder as ConnectionBuilder, fdo::ObjectManagerProxy,
    };
    use zbus::zvariant::{ObjectPath, OwnedObjectPath};

    const IWD_SERVICE: &str = "net.connman.iwd";
    const STATION_INTERFACE: &str = "net.connman.iwd.Station";
    const NETWORK_INTERFACE: &str = "net.connman.iwd.Network";
    const AGENT_MANAGER_INTERFACE: &str = "net.connman.iwd.AgentManager";
    const AGENT_PATH: &str = "/io/palodurosystems/LatticeTablet/NetworkAgent";

    #[derive(Debug)]
    struct PassphraseAgent {
        passphrase: Arc<Mutex<Option<String>>>,
    }

    #[zbus::interface(name = "net.connman.iwd.Agent")]
    impl PassphraseAgent {
        fn release(&self) {
            if let Ok(mut passphrase) = self.passphrase.lock() {
                passphrase.take();
            }
        }

        fn request_passphrase(&self, _network: ObjectPath<'_>) -> zbus::fdo::Result<String> {
            self.passphrase
                .lock()
                .map_err(|_| zbus::fdo::Error::Failed("network agent state unavailable".into()))?
                .clone()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| zbus::fdo::Error::Failed("network passphrase is required".into()))
        }

        fn cancel(&self, _reason: &str) {
            if let Ok(mut passphrase) = self.passphrase.lock() {
                passphrase.take();
            }
        }
    }

    pub(super) fn run(receiver: Receiver<NetworkCommand>, snapshot: Arc<RwLock<NetworkSnapshot>>) {
        let passphrase = Arc::new(Mutex::new(None));
        let connection = match ConnectionBuilder::system().and_then(|builder| {
            builder
                .serve_at(
                    AGENT_PATH,
                    PassphraseAgent {
                        passphrase: Arc::clone(&passphrase),
                    },
                )?
                .build()
        }) {
            Ok(connection) => connection,
            Err(error) => {
                set_failure(&snapshot, format!("Wireless service unavailable: {error}"));
                while receiver.recv().is_ok() {}
                return;
            }
        };

        let mut agent_registered = false;
        loop {
            if !agent_registered {
                agent_registered = register_agent(&connection).is_ok();
            }

            if let Err(error) = refresh(&connection, &snapshot) {
                set_failure(&snapshot, format!("Wireless service unavailable: {error}"));
            }

            match receiver.recv_timeout(Duration::from_secs(2)) {
                Ok(NetworkCommand::Scan) => {
                    set_activity(&snapshot, "Scanning for networks", true);
                    if let Err(error) = scan(&connection) {
                        set_failure(&snapshot, format!("Network scan failed: {error}"));
                    }
                }
                Ok(NetworkCommand::Connect {
                    id,
                    passphrase: key,
                }) => {
                    set_activity(&snapshot, "Connecting", false);
                    if let Ok(mut stored) = passphrase.lock() {
                        *stored = Some(key);
                    }
                    if let Err(error) = connect(&connection, &id) {
                        set_failure(&snapshot, format!("Connection failed: {error}"));
                    }
                    if let Ok(mut stored) = passphrase.lock() {
                        stored.take();
                    }
                }
                Ok(NetworkCommand::Disconnect) => {
                    set_activity(&snapshot, "Disconnecting", false);
                    if let Err(error) = disconnect(&connection) {
                        set_failure(&snapshot, format!("Disconnect failed: {error}"));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn register_agent(connection: &Connection) -> zbus::Result<()> {
        let proxy = Proxy::new(
            connection,
            IWD_SERVICE,
            "/net/connman/iwd",
            AGENT_MANAGER_INTERFACE,
        )?;
        proxy.call("RegisterAgent", &(ObjectPath::try_from(AGENT_PATH)?,))
    }

    fn managed_objects(connection: &Connection) -> zbus::Result<zbus::fdo::ManagedObjects> {
        let proxy = ObjectManagerProxy::builder(connection)
            .destination(IWD_SERVICE)?
            .path("/")?
            .build()?;
        proxy.get_managed_objects().map_err(Into::into)
    }

    fn station_path(connection: &Connection) -> zbus::Result<OwnedObjectPath> {
        managed_objects(connection)?
            .into_iter()
            .find_map(|(path, interfaces)| {
                interfaces
                    .keys()
                    .any(|name| name.as_str() == STATION_INTERFACE)
                    .then_some(path)
            })
            .ok_or_else(|| zbus::Error::Failure("no wireless station is available".to_string()))
    }

    fn station_proxy<'a>(
        connection: &'a Connection,
        path: &'a OwnedObjectPath,
    ) -> zbus::Result<Proxy<'a>> {
        Proxy::new(connection, IWD_SERVICE, path.as_str(), STATION_INTERFACE)
    }

    fn scan(connection: &Connection) -> zbus::Result<()> {
        let path = station_path(connection)?;
        station_proxy(connection, &path)?.call("Scan", &())
    }

    fn connect(connection: &Connection, id: &str) -> zbus::Result<()> {
        let path = ObjectPath::try_from(id)?;
        Proxy::new(connection, IWD_SERVICE, path.as_str(), NETWORK_INTERFACE)?.call("Connect", &())
    }

    fn disconnect(connection: &Connection) -> zbus::Result<()> {
        let path = station_path(connection)?;
        station_proxy(connection, &path)?.call("Disconnect", &())
    }

    fn refresh(
        connection: &Connection,
        snapshot: &Arc<RwLock<NetworkSnapshot>>,
    ) -> zbus::Result<()> {
        let path = station_path(connection)?;
        let station = station_proxy(connection, &path)?;
        let scanning = station.get_property::<bool>("Scanning")?;
        let station_state = station.get_property::<String>("State")?;
        let ordered: Vec<(OwnedObjectPath, i16)> = station.call("GetOrderedNetworks", &())?;
        let mut networks = Vec::with_capacity(ordered.len());

        for (path, raw_signal) in ordered {
            let network = Proxy::new(connection, IWD_SERVICE, path.as_str(), NETWORK_INTERFACE)?;
            networks.push(NetworkEntry {
                id: path.to_string(),
                name: network.get_property("Name")?,
                security: network.get_property("Type")?,
                signal: signal_percent(raw_signal),
                connected: network.get_property("Connected")?,
            });
        }

        let connected_name = networks
            .iter()
            .find(|network| network.connected)
            .map(|network| network.name.as_str());
        let status = connected_name.map_or_else(
            || match station_state.as_str() {
                "connecting" | "connecting-auto" | "netconfig" => "Connecting".to_string(),
                "roaming" => "Roaming".to_string(),
                _ => "Not connected".to_string(),
            },
            |name| format!("Connected to {name}"),
        );

        if let Ok(mut state) = snapshot.write() {
            *state = NetworkSnapshot {
                available: true,
                scanning,
                status,
                networks,
            };
        }
        Ok(())
    }

    pub(super) fn signal_percent(raw: i16) -> i32 {
        let dbm = if raw < -200 {
            i32::from(raw) / 100
        } else {
            i32::from(raw)
        };
        ((dbm + 90) * 100 / 60).clamp(0, 100)
    }

    fn set_activity(snapshot: &Arc<RwLock<NetworkSnapshot>>, status: &str, scanning: bool) {
        if let Ok(mut state) = snapshot.write() {
            state.available = true;
            state.scanning = scanning;
            state.status = status.to_string();
        }
    }

    fn set_failure(snapshot: &Arc<RwLock<NetworkSnapshot>>, status: String) {
        if let Ok(mut state) = snapshot.write() {
            state.available = false;
            state.scanning = false;
            state.status = status;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_has_no_invented_networks() {
        let snapshot = NetworkSnapshot::default();
        assert!(!snapshot.available);
        assert!(snapshot.networks.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn signal_strength_is_bounded() {
        assert_eq!(linux::signal_percent(-10_000), 0);
        assert_eq!(linux::signal_percent(-9_000), 0);
        assert_eq!(linux::signal_percent(-6_000), 50);
        assert_eq!(linux::signal_percent(-3_000), 100);
        assert_eq!(linux::signal_percent(0), 100);
    }
}
