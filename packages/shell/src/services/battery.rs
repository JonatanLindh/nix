//! The laptop battery from UPower's display device.
//!
//! A small zbus 5 proxy rather than the `upower_dbus` crate, which is still on zbus 3 and would
//! pull in a second copy next to nmrs and logind-zbus.

use std::time::Duration;

use futures::channel::mpsc::Sender;
use futures::{SinkExt, StreamExt, stream};

use super::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub percent: u32,
    pub charging: bool,
    /// Plugged in: charging, full, or holding charge.
    pub on_ac: bool,
    /// Seconds until full while charging, or until empty otherwise; 0 when UPower doesn't know.
    pub seconds_left: i64,
}

impl State {
    pub fn low(&self) -> bool {
        !self.on_ac && self.percent <= 15
    }

    /// "1h 20m left", "45m to full", or `None` when UPower has no estimate.
    pub fn time_left(&self) -> Option<String> {
        let s = self.seconds_left;
        if s <= 0 {
            return None;
        }
        let (h, m) = (s / 3600, ((s % 3600) as f64 / 60.).round() as i64);
        let hours = if h > 0 {
            format!("{h}h ")
        } else {
            String::new()
        };
        let what = if self.charging { "to full" } else { "left" };
        Some(format!("{hours}{m}m {what}"))
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait Device {
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;
}

// UPower's Device.Type and Device.State values.
const TYPE_BATTERY: u32 = 2;
const CHARGING: u32 = 1;
const FULLY_CHARGED: u32 = 4;
const PENDING_CHARGE: u32 = 5;
const PENDING_DISCHARGE: u32 = 6;

/// `None` on a machine without a laptop battery (or without UPower).
pub fn watch() -> futures::channel::mpsc::Receiver<Option<State>> {
    crate::runtime::stream(4, async |out| {
        super::supervise("battery", Duration::from_secs(30), out, run).await
    })
}

async fn run(mut out: Sender<Option<State>>) -> Result {
    let connection = zbus::Connection::system().await?;
    let device = DeviceProxy::new(&connection).await?;
    let changes = stream::select(
        device.receive_percentage_changed().await.map(|_| ()),
        stream::select(
            device.receive_state_changed().await.map(|_| ()),
            stream::select(
                device.receive_time_to_empty_changed().await.map(|_| ()),
                device.receive_time_to_full_changed().await.map(|_| ()),
            ),
        ),
    );
    let mut changes = std::pin::pin!(changes);
    loop {
        out.send(snapshot(&device).await.ok().flatten()).await?;
        changes.next().await.ok_or("UPower went away")?;
    }
}

async fn snapshot(device: &DeviceProxy<'_>) -> Result<Option<State>> {
    let laptop = device.kind().await? == TYPE_BATTERY
        && device.is_present().await?
        && device.power_supply().await?;
    if !laptop {
        return Ok(None);
    }
    let state = device.state().await?;
    let charging = matches!(state, CHARGING | PENDING_CHARGE);
    Ok(Some(State {
        percent: device.percentage().await?.round() as u32,
        charging,
        on_ac: charging || matches!(state, FULLY_CHARGED | PENDING_DISCHARGE),
        seconds_left: if charging {
            device.time_to_full().await?
        } else {
            device.time_to_empty().await?
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(charging: bool, seconds_left: i64) -> State {
        State {
            percent: 50,
            charging,
            on_ac: charging,
            seconds_left,
        }
    }

    #[test]
    fn tells_time_left() {
        assert_eq!(battery(false, 0).time_left(), None);
        assert_eq!(
            battery(false, 4800).time_left().as_deref(),
            Some("1h 20m left")
        );
        assert_eq!(
            battery(true, 2700).time_left().as_deref(),
            Some("45m to full")
        );
    }
}
