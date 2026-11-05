//! Connectivity from NetworkManager, via nmrs: wired links, Wi-Fi and the networks in range.

use std::time::Duration;

use futures::channel::mpsc::Sender;
use futures::{SinkExt, StreamExt};
use nmrs::{DeviceState, DeviceType, NetworkManager, WifiSecurity};

use super::Result;
use crate::theme;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub wired: Vec<Wired>,
    /// `None` on a machine without Wi-Fi.
    pub wifi: Option<Wifi>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wired {
    pub interface: String,
    pub connected: bool,
    /// A cable is in (connected or not).
    pub has_link: bool,
    pub speed_mbps: Option<u32>,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wifi {
    pub interface: String,
    pub enabled: bool,
    /// The network it's connected to.
    pub current: Option<Network>,
    pub ip: Option<String>,
    /// Other networks in range: saved ones first, then by signal.
    pub others: Vec<Network>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    pub ssid: String,
    /// 0–100.
    pub strength: u8,
    pub secured: bool,
    /// NetworkManager has a profile for it, so it connects without asking.
    pub known: bool,
    pub frequency: Option<u32>,
}

impl Network {
    pub fn icon(&self) -> &'static str {
        match self.strength {
            60.. => "wifi",
            35.. => "wifi-2",
            _ => "wifi-1",
        }
    }

    /// "5 GHz".
    pub fn band(&self) -> Option<&'static str> {
        match self.frequency? {
            5925.. => Some("6 GHz"),
            4900.. => Some("5 GHz"),
            1.. => Some("2.4 GHz"),
            0 => None,
        }
    }
}

impl State {
    pub fn wired_up(&self) -> bool {
        self.wired.iter().any(|w| w.connected)
    }

    pub fn online(&self) -> bool {
        self.wired_up() || self.wifi.as_ref().is_some_and(|w| w.current.is_some())
    }

    pub fn icon(&self) -> &'static str {
        match &self.wifi {
            _ if self.wired_up() => "ethernet",
            Some(Wifi {
                current: Some(n), ..
            }) => match n.strength {
                67.. => "wifi",
                34.. => "wifi-2",
                _ => "wifi-1",
            },
            Some(_) => "wifi-off",
            // A machine without Wi-Fi shows its unplugged cable instead.
            None => "ethernet",
        }
    }

    /// Wi-Fi switched off is a choice; anything else offline is a problem.
    pub fn color(&self) -> gpui::Rgba {
        match &self.wifi {
            _ if self.online() => theme::TEXT,
            Some(Wifi { enabled: false, .. }) => theme::TEXT_FAINT,
            _ => theme::WARN_TEXT,
        }
    }
}

pub fn watch() -> futures::channel::mpsc::Receiver<State> {
    crate::runtime::stream(4, async |out| {
        super::supervise("network", Duration::from_secs(5), out, run).await
    })
}

async fn run(mut out: Sender<State>) -> Result {
    let nm = NetworkManager::new().await?;
    let mut events = nm.network_events().await?;
    loop {
        out.send(snapshot(&nm).await?).await?;
        events
            .next()
            .await
            .ok_or("NetworkManager event stream ended")??;
        // The stream is lossy and bursty; settle before re-reading.
        while let Ok(Some(e)) =
            tokio::time::timeout(Duration::from_millis(150), events.next()).await
        {
            e?;
        }
    }
}

async fn snapshot(nm: &NetworkManager) -> Result<State> {
    let devices = nm.list_devices().await?;
    let ip = |iface: &str| {
        devices
            .iter()
            .find(|d| d.interface == iface)
            .and_then(|d| d.ip4_address.as_deref())
            .map(|a| a.split('/').next().unwrap_or(a).to_string())
    };
    let wired = nm
        .list_wired_device_details()
        .await?
        .into_iter()
        .map(|d| Wired {
            ip: ip(&d.interface),
            connected: d.state == DeviceState::Activated,
            has_link: !matches!(d.state, DeviceState::Unavailable | DeviceState::Unmanaged),
            speed_mbps: d.speed_mbps.filter(|&s| s > 0),
            interface: d.interface,
        })
        .collect();

    let wifi = match devices.iter().find(|d| d.device_type == DeviceType::Wifi) {
        None => None,
        Some(device) => {
            let enabled = nm.wifi_state().await?.enabled;
            let mut current = None;
            let mut others = Vec::new();
            if enabled {
                for n in nm.list_networks(Some(&device.interface)).await? {
                    let network = Network {
                        strength: n.strength.unwrap_or(0),
                        secured: n.secured,
                        known: n.known,
                        frequency: n.frequency,
                        ssid: n.ssid,
                    };
                    if network.ssid.is_empty() {
                        continue;
                    }
                    if n.is_active {
                        current = Some(network);
                    } else if !others.iter().any(|o: &Network| o.ssid == network.ssid) {
                        others.push(network);
                    }
                }
            }
            others.sort_by(|a, b| b.known.cmp(&a.known).then(b.strength.cmp(&a.strength)));
            others.truncate(8);
            Some(Wifi {
                ip: ip(&device.interface),
                interface: device.interface.clone(),
                enabled,
                current,
                others,
            })
        }
    };
    Ok(State { wired, wifi })
}

async fn manager() -> std::result::Result<NetworkManager, String> {
    NetworkManager::new().await.map_err(|e| e.to_string())
}

pub async fn set_wifi(enabled: bool) {
    if let Err(e) = async {
        manager()
            .await?
            .set_wireless_enabled(enabled)
            .await
            .map_err(|e| e.to_string())
    }
    .await
    {
        eprintln!("network: wifi {enabled}: {e}");
    }
}

/// Connects to `ssid`: with its saved profile if it has one, else with `password` (none for an
/// open network).
pub async fn connect(ssid: String, password: Option<String>) -> std::result::Result<(), String> {
    let security = match password {
        Some(psk) => WifiSecurity::WpaPsk { psk },
        None => WifiSecurity::Open,
    };
    manager()
        .await?
        .connect(&ssid, None, security)
        .await
        .map_err(|e| e.to_string())
}

pub async fn disconnect(interface: String) {
    if let Err(e) = async {
        manager()
            .await?
            .disconnect(Some(&interface))
            .await
            .map_err(|e| e.to_string())
    }
    .await
    {
        eprintln!("network: disconnect: {e}");
    }
}

pub async fn rescan(interface: String) {
    if let Err(e) = async {
        manager()
            .await?
            .scan_networks(Some(&interface))
            .await
            .map_err(|e| e.to_string())
    }
    .await
    {
        eprintln!("network: rescan: {e}");
    }
}
