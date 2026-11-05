//! Caffeine: keep the screen awake with a logind idle inhibitor, which hypridle honours. The lock
//! lasts as long as the returned file descriptor stays open.

use std::os::fd::OwnedFd;
use std::sync::Arc;

use logind_zbus::manager::{InhibitType, ManagerProxy};

/// The held inhibitor. Dropping the last clone releases it.
pub type Inhibitor = Arc<OwnedFd>;

pub async fn inhibit() -> Option<Inhibitor> {
    let result = async {
        let connection = zbus::Connection::system().await?;
        let manager = ManagerProxy::new(&connection).await?;
        let fd = manager
            .inhibit(InhibitType::Idle, "shell", "Caffeine", "block")
            .await?;
        zbus::Result::Ok(Arc::new(OwnedFd::from(fd)))
    };
    result
        .await
        .inspect_err(|e| eprintln!("caffeine: {e}"))
        .ok()
}

/// Caffeine's switch: on while the inhibitor is held.
#[derive(Default)]
pub struct Caffeine {
    held: Option<Inhibitor>,
    /// Asked logind, no answer yet.
    pending: bool,
}

impl Caffeine {
    pub fn on(&self) -> bool {
        self.held.is_some()
    }

    pub fn toggle(&mut self, cx: &mut gpui::Context<Self>) {
        if self.held.take().is_some() {
            cx.notify();
            return;
        }
        if self.pending {
            return;
        }
        self.pending = true;
        let request = crate::runtime::spawn(inhibit());
        cx.spawn(async move |this, cx| {
            let held = request.await.ok().flatten();
            this.update(cx, |caffeine, cx| {
                caffeine.pending = false;
                caffeine.held = held;
                cx.notify();
            })
        })
        .detach();
    }
}
