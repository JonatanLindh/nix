//! The local time, once a minute on the minute.

use std::time::Duration;

use chrono::{DateTime, Local, TimeDelta, Timelike};
use futures::SinkExt;
use futures::channel::mpsc::Receiver;

pub fn watch() -> Receiver<DateTime<Local>> {
    crate::runtime::stream(1, async |mut out| {
        loop {
            let now = Local::now();
            if out.send(now).await.is_err() {
                return;
            }
            // Wake just after the next minute starts.
            let into_minute = TimeDelta::seconds(now.second().into())
                + TimeDelta::nanoseconds(now.nanosecond().into());
            let wait = (TimeDelta::minutes(1) - into_minute)
                .to_std()
                .unwrap_or_default();
            tokio::time::sleep(wait + Duration::from_millis(5)).await;
        }
    })
}
