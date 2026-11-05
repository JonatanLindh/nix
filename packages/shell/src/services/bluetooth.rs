//! The default Bluetooth adapter, via bluer: whether it's on, its devices (for the bar's icon and
//! BtPanel.qml), and the actions on them.

use std::pin::Pin;
use std::time::Duration;

use bluer::{AdapterEvent, AdapterProperty, Address, DeviceEvent, DeviceProperty};
use futures::channel::mpsc::Sender;
use futures::stream::SelectAll;
use futures::{SinkExt, Stream, StreamExt};

use super::Result;
use crate::theme;

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub powered: bool,
    /// Scanning for devices nearby.
    pub discovering: bool,
    /// Those with a name, by name.
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub address: Address,
    pub name: String,
    /// What it is, as one of our icons.
    pub icon: &'static str,
    pub connected: bool,
    pub paired: bool,
    /// Charge in percent, when it reports one.
    pub battery: Option<u8>,
}

impl State {
    pub fn connected(&self) -> bool {
        self.devices.iter().any(|d| d.connected)
    }

    /// The bar icon's colour: faint while off, blue while something is connected.
    pub fn color(&self) -> gpui::Rgba {
        if !self.powered {
            theme::alpha(theme::TEXT, 0.42)
        } else if self.connected() {
            theme::BLUETOOTH
        } else {
            theme::TEXT
        }
    }
}

/// BlueZ's icon name (freedesktop names, e.g. "audio-headset") as one of our icons.
pub fn device_icon(bluez: &str) -> &'static str {
    let has = |words: &[&str]| words.iter().any(|w| bluez.contains(w));
    if has(&["headset", "headphone"]) {
        "headphones"
    } else if has(&["speaker", "audio-card"]) {
        "speaker"
    } else if has(&["mouse", "tablet"]) {
        "mouse"
    } else if has(&["keyboard"]) {
        "keyboard"
    } else if has(&["gaming", "joystick"]) {
        "gamepad"
    } else if has(&["phone"]) {
        "phone"
    } else if has(&["computer", "laptop"]) {
        "laptop"
    } else {
        "bluetooth"
    }
}

/// `None` while there is no adapter.
pub fn watch() -> futures::channel::mpsc::Receiver<Option<State>> {
    crate::runtime::stream(4, async |out| {
        super::supervise("bluetooth", Duration::from_secs(10), out, run).await
    })
}

type DeviceEvents = Pin<Box<dyn Stream<Item = DeviceEvent> + Send>>;

async fn run(mut out: Sender<Option<State>>) -> Result {
    let session = bluer::Session::new().await?;
    let adapter = match session.default_adapter().await {
        Ok(adapter) => adapter,
        Err(e) => {
            out.send(None).await?;
            return Err(e.into());
        }
    };

    let mut adapter_events = adapter.events().await?;
    // Device changes come per device, so watch each one.
    let mut device_events: SelectAll<DeviceEvents> = SelectAll::new();
    for address in adapter.device_addresses().await? {
        device_events.push(Box::pin(adapter.device(address)?.events().await?));
    }

    loop {
        out.send(Some(snapshot(&adapter).await?)).await?;
        loop {
            tokio::select! {
                event = adapter_events.next() => match event.ok_or("adapter went away")? {
                    AdapterEvent::DeviceAdded(address) => {
                        device_events.push(Box::pin(adapter.device(address)?.events().await?));
                        break;
                    }
                    AdapterEvent::DeviceRemoved(_)
                    | AdapterEvent::PropertyChanged(
                        AdapterProperty::Powered(_) | AdapterProperty::Discovering(_),
                    ) => break,
                    _ => {}
                },
                Some(DeviceEvent::PropertyChanged(change)) = device_events.next(),
                    if !device_events.is_empty() =>
                {
                    // Signal strength and the like change all the time while scanning.
                    if matches!(
                        change,
                        DeviceProperty::Connected(_)
                            | DeviceProperty::Paired(_)
                            | DeviceProperty::Name(_)
                            | DeviceProperty::Alias(_)
                            | DeviceProperty::Icon(_)
                            | DeviceProperty::BatteryPercentage(_)
                    ) {
                        break;
                    }
                }
            }
        }
    }
}

async fn snapshot(adapter: &bluer::Adapter) -> Result<State> {
    let mut devices = Vec::new();
    for address in adapter.device_addresses().await? {
        let device = adapter.device(address)?;
        // Unnamed devices nearby are only addresses; nothing to show for them.
        if device.name().await?.is_none() {
            continue;
        }
        devices.push(Device {
            address,
            name: device.alias().await?,
            icon: device_icon(&device.icon().await?.unwrap_or_default()),
            connected: device.is_connected().await?,
            paired: device.is_paired().await?,
            battery: device.battery_percentage().await.ok().flatten(),
        });
    }
    devices.sort_by_key(|d| d.name.to_lowercase());
    Ok(State {
        powered: adapter.is_powered().await?,
        discovering: adapter.is_discovering().await?,
        devices,
    })
}

async fn adapter() -> bluer::Result<bluer::Adapter> {
    bluer::Session::new().await?.default_adapter().await
}

/// Runs `action`, logging a failure; `Ok` when it worked.
async fn logged<F>(what: &str, action: F) -> std::result::Result<(), ()>
where
    F: Future<Output = bluer::Result<()>>,
{
    action
        .await
        .map_err(|e| eprintln!("bluetooth: {what}: {e}"))
}

pub async fn set_powered(on: bool) {
    let _ = logged("switching", async {
        adapter().await?.set_powered(on).await
    })
    .await;
}

pub async fn connect(address: Address) -> std::result::Result<(), ()> {
    logged("connecting", async {
        adapter().await?.device(address)?.connect().await
    })
    .await
}

pub async fn disconnect(address: Address) {
    let _ = logged("disconnecting", async {
        adapter().await?.device(address)?.disconnect().await
    })
    .await;
}

/// Pairs (BlueZ asks the registered agent for a PIN if the device needs one).
pub async fn pair(address: Address) -> std::result::Result<(), ()> {
    logged("pairing", async {
        adapter().await?.device(address)?.pair().await
    })
    .await
}

/// A scan for devices nearby, for as long as it's held. BlueZ stops discovering once nobody
/// holds a scan.
pub struct Scan(tokio::task::JoinHandle<()>);

impl Drop for Scan {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub fn scan() -> Scan {
    Scan(crate::runtime::spawn(async {
        let result = async {
            let adapter = adapter().await?;
            let mut found = std::pin::pin!(adapter.discover_devices().await?);
            // What's found reaches the state through the adapter's own events; this only keeps
            // the discovery session open until the stream ends or the scan is dropped.
            while found.next().await.is_some() {}
            bluer::Result::Ok(())
        };
        if let Err(e) = result.await {
            eprintln!("bluetooth: scanning: {e}");
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_icons() {
        assert_eq!(device_icon("audio-headset"), "headphones");
        assert_eq!(device_icon("audio-headphones"), "headphones");
        assert_eq!(device_icon("audio-card"), "speaker");
        assert_eq!(device_icon("input-mouse"), "mouse");
        assert_eq!(device_icon("input-keyboard"), "keyboard");
        assert_eq!(device_icon("input-gaming"), "gamepad");
        assert_eq!(device_icon("phone"), "phone");
        assert_eq!(device_icon("computer"), "laptop");
        assert_eq!(device_icon(""), "bluetooth");
    }
}
