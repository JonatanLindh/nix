//! The power profile (Power.qml), from power-profiles-daemon's D-Bus API (tlp-pd speaks it too):
//! which profile is active and which exist, and switching between them. The desktop has no such
//! daemon, which shows as `None`.

use std::collections::HashMap;
use std::time::Duration;

use futures::channel::mpsc::Sender;
use futures::{SinkExt, StreamExt};
use zbus::zvariant::OwnedValue;

use super::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
    PowerSaver,
    Balanced,
    Performance,
}

impl Profile {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "power-saver" => Some(Self::PowerSaver),
            "balanced" => Some(Self::Balanced),
            "performance" => Some(Self::Performance),
            _ => None,
        }
    }

    /// As the daemon calls it.
    pub fn name(self) -> &'static str {
        match self {
            Self::PowerSaver => "power-saver",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PowerSaver => "Saver",
            Self::Balanced => "Balanced",
            Self::Performance => "Performance",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::PowerSaver => "leaf",
            Self::Balanced => "gauge",
            Self::Performance => "bolt",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub active: Profile,
    /// Saver to performance.
    pub profiles: Vec<Profile>,
}

/// The daemon under its current name, then its older one.
const SERVICES: [(&str, &str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
        "org.freedesktop.UPower.PowerProfiles",
    ),
    (
        "net.hadess.PowerProfiles",
        "/net/hadess/PowerProfiles",
        "net.hadess.PowerProfiles",
    ),
];

async fn proxy(connection: &zbus::Connection) -> Result<zbus::Proxy<'static>> {
    let mut error = None;
    for (service, path, interface) in SERVICES {
        let proxy = zbus::Proxy::new(connection, service, path, interface).await?;
        // A proxy is made for any name; only a property read tells if the daemon is there.
        match proxy.get_property::<String>("ActiveProfile").await {
            Ok(_) => return Ok(proxy),
            Err(e) => error = Some(e),
        }
    }
    Err(error.map_or_else(|| "no power profiles daemon".into(), Into::into))
}

/// `None` while there's no profiles daemon.
pub fn watch() -> futures::channel::mpsc::Receiver<Option<State>> {
    crate::runtime::stream(4, async |out| {
        super::supervise("power profiles", Duration::from_secs(30), out, run).await
    })
}

async fn run(mut out: Sender<Option<State>>) -> Result {
    let connection = zbus::Connection::system().await?;
    let proxy = match proxy(&connection).await {
        Ok(proxy) => proxy,
        Err(e) => {
            out.send(None).await?;
            return Err(e);
        }
    };
    let mut changes = futures::stream::select(
        proxy
            .receive_property_changed::<String>("ActiveProfile")
            .await
            .map(|_| ()),
        proxy
            .receive_property_changed::<OwnedValue>("Profiles")
            .await
            .map(|_| ()),
    );
    loop {
        let active = proxy.get_property::<String>("ActiveProfile").await?;
        let listed: Vec<HashMap<String, OwnedValue>> = proxy.get_property("Profiles").await?;
        let names = listed
            .iter()
            .filter_map(|p| p.get("Profile"))
            .filter_map(|v| String::try_from(v.clone()).ok());
        out.send(state(&active, names)).await?;
        changes
            .next()
            .await
            .ok_or("power profiles daemon went away")?;
    }
}

fn state(active: &str, names: impl Iterator<Item = String>) -> Option<State> {
    let mut profiles: Vec<Profile> = names.filter_map(|n| Profile::from_name(&n)).collect();
    profiles.sort();
    profiles.dedup();
    Some(State {
        active: Profile::from_name(active)?,
        profiles,
    })
}

async fn set(profile: Profile) {
    let result = async {
        let connection = zbus::Connection::system().await?;
        let proxy = proxy(&connection).await?;
        proxy.set_property("ActiveProfile", profile.name()).await?;
        Result::Ok(())
    };
    if let Err(e) = result.await {
        eprintln!("power profiles: setting {}: {e}", profile.name());
    }
}

/// The profile on the UI side: the daemon's, or the one just picked while the daemon catches up.
pub struct PowerProfiles {
    /// `None` while there's no daemon.
    pub state: Option<State>,
    pending: Option<Profile>,
    /// Bumped on each pick, so only the last one's timers act.
    picks: u64,
}

/// Rapid clicks only move the shown profile; the daemon gets the final choice once they stop,
/// since tlp-pd applies (and reports back) every profile it's sent, in order.
const APPLY_AFTER: Duration = Duration::from_millis(400);
/// The pick is shown until the daemon has caught up.
const SETTLE: Duration = Duration::from_secs(3);

impl PowerProfiles {
    pub fn new(cx: &mut gpui::App) -> gpui::Entity<Self> {
        use gpui::AppContext;
        cx.new(|cx: &mut gpui::Context<Self>| {
            cx.spawn(async move |this, cx| {
                let mut reports = watch();
                while let Some(state) = reports.next().await {
                    let alive = this.update(cx, |p, cx| {
                        if p.state != state {
                            p.state = state;
                            cx.notify();
                        }
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
            Self {
                state: None,
                pending: None,
                picks: 0,
            }
        })
    }

    /// The profile to show.
    pub fn profile(&self) -> Option<Profile> {
        self.pending
            .or_else(|| self.state.as_ref().map(|s| s.active))
    }

    pub fn set(&mut self, profile: Profile, cx: &mut gpui::Context<Self>) {
        self.pending = Some(profile);
        self.picks += 1;
        let pick = self.picks;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(APPLY_AFTER).await;
            let latest = this.read_with(cx, |p, _| p.picks == pick).unwrap_or(false);
            if !latest {
                return;
            }
            crate::runtime::detach(set(profile));
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |p, cx| {
                if p.picks == pick {
                    p.pending = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_profiles() {
        let names = ["performance", "balanced", "power-saver", "custom"].map(String::from);
        let read = state("balanced", names.into_iter()).unwrap();
        assert_eq!(read.active, Profile::Balanced);
        assert_eq!(
            read.profiles,
            [Profile::PowerSaver, Profile::Balanced, Profile::Performance]
        );
        assert!(state("custom", std::iter::empty()).is_none());
        for p in read.profiles {
            assert_eq!(Profile::from_name(p.name()), Some(p));
        }
    }
}
