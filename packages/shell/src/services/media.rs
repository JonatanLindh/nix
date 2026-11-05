//! What's playing: the MPRIS players on the session bus, after Quickshell's own MPRIS service.
//!
//! Players come and go with their `org.mpris.MediaPlayer2.*` names; each one is followed by a
//! task of its own that keeps its properties current from `PropertiesChanged`. The track shown is
//! picked from them as NowPlaying.qml does (see [`choose`]).
//!
//! MPRIS doesn't signal the position as it moves, so a [`Position`] is the last reported one and
//! when it was reported, and the UI estimates the current one from the time since. It's read
//! again whenever that estimate could go wrong: on `Seeked`, on play/pause, rate and track
//! changes, and after we seek.
//!
//! Cover art is a local file or a web URL (Spotify's are); web art is fetched once per track.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use futures::channel::mpsc::Sender;
use futures::{SinkExt, StreamExt};
use zbus::names::InterfaceName;
use zbus::proxy::CacheProperties;
use zbus::zvariant::{ObjectPath, Value};

use super::{Result, icons};

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// The player's bus name, which the controls go to.
    pub player: String,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub art: Option<icons::Image>,
    /// `None` when the player doesn't say (a live stream, say).
    pub length: Option<Duration>,
    /// `None` when the player doesn't report one.
    pub position: Option<Position>,
    /// `mpris:trackid`, which `SetPosition` wants.
    track_id: Option<String>,
    pub can_seek: bool,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_play: bool,
    pub can_pause: bool,
}

impl Track {
    /// Where playback is at `now`, estimated from the last report and kept within the track.
    pub fn position_at(&self, now: Instant) -> Option<Duration> {
        let position = self.position?.at(now);
        Some(self.length.map_or(position, |length| position.min(length)))
    }

    /// Whether there's a length and a position to show progress with.
    pub fn has_progress(&self) -> bool {
        self.position.is_some() && self.length.is_some_and(|l| !l.is_zero())
    }

    /// Takes `to` as the position from `now`, as a seek there will make it, so the card
    /// doesn't jump back while the player catches up.
    pub fn assume_seeked(&mut self, to: Duration, now: Instant) {
        if let Some(position) = &mut self.position {
            *position = Position::new(to, now, position.rate, position.playing);
        }
    }
}

/// A reported position, from which the current one is estimated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    reported: Duration,
    /// When it was reported.
    at: Instant,
    rate: f64,
    playing: bool,
}

impl Position {
    pub fn new(reported: Duration, at: Instant, rate: f64, playing: bool) -> Self {
        Self {
            reported,
            at,
            rate,
            playing,
        }
    }

    /// The position at `now`: the reported one, plus the time since × rate while playing.
    pub fn at(&self, now: Instant) -> Duration {
        if !self.playing {
            return self.reported;
        }
        // `max` also turns a NaN rate into 0.
        let moved = now.saturating_duration_since(self.at).as_secs_f64() * self.rate.max(0.);
        self.reported + Duration::try_from_secs_f64(moved).unwrap_or_default()
    }

    /// How long until the position reaches its next whole second, for a clock showing seconds
    /// to tick on time. `None` while it isn't moving.
    pub fn until_next_second(&self, now: Instant) -> Option<Duration> {
        if !self.playing || self.rate.is_nan() || self.rate <= 0. {
            return None;
        }
        let into = f64::from(self.at(now).subsec_nanos()) / 1e9;
        // A little past the boundary, so the floor has moved by then; never longer than a
        // second, however slow the rate.
        let wait = ((1. - into) / self.rate).min(1.) + 0.01;
        Some(Duration::from_secs_f64(wait))
    }

    /// Takes the estimate at `now` as a new report with `rate` and `playing`, so a change of
    /// either counts from the moment it happened.
    fn retime(&mut self, now: Instant, rate: f64, playing: bool) {
        *self = Self::new(self.at(now), now, rate, playing);
    }
}

/// The current track, or `None` while no player has one.
pub fn watch() -> futures::channel::mpsc::Receiver<Option<Track>> {
    crate::runtime::stream(4, async |out| {
        super::supervise("media", Duration::from_secs(5), out, run).await
    })
}

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
/// The `mpris:trackid` meaning there's no track, which `SetPosition` can't take.
const NO_TRACK: &str = "/org/mpris/MediaPlayer2/TrackList/NoTrack";
/// Some players (YouTube in a browser) update their position a while after their status, so
/// it's read once more after this.
const SETTLE: Duration = Duration::from_millis(150);

#[zbus::proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    fn play_pause(&self) -> zbus::Result<()>;
    fn seek(&self, offset: i64) -> zbus::Result<()>;
    fn set_position(&self, track_id: &ObjectPath<'_>, position: i64) -> zbus::Result<()>;
    #[zbus(signal)]
    fn seeked(&self, position: i64) -> zbus::Result<()>;
    #[zbus(property)]
    fn position(&self) -> zbus::Result<i64>;
}

/// A seek we made, for the player's task to take as its position right away.
#[derive(Debug, Clone)]
struct Seek {
    player: String,
    to: Duration,
}

static SEEKS: LazyLock<tokio::sync::watch::Sender<Option<Seek>>> =
    LazyLock::new(|| tokio::sync::watch::Sender::new(None));

/// A player's task reporting its state; the generation tells it from an earlier owner's.
type Report = (String, u64, Player);

struct Entry {
    name: String,
    generation: u64,
    /// `None` until the player's first report.
    state: Option<Player>,
    /// The art URL last fetched for it, and what came of it.
    art: Option<(String, Option<icons::Image>)>,
    task: tokio::task::AbortHandle,
}

impl Drop for Entry {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run(mut out: Sender<Option<Track>>) -> Result {
    let connection = zbus::Connection::session().await?;
    let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
    let mut owners = dbus.receive_name_owner_changed().await?;
    let (reports, mut incoming) = tokio::sync::mpsc::channel::<Report>(16);
    // In the order they appeared, which is what "the first player" means.
    let mut players: Vec<Entry> = Vec::new();
    let mut generation = 0;
    let mut add = |players: &mut Vec<Entry>, name: String| {
        generation += 1;
        let task = tokio::spawn(follow(
            connection.clone(),
            name.clone(),
            generation,
            reports.clone(),
        ));
        players.push(Entry {
            name,
            generation,
            state: None,
            art: None,
            task: task.abort_handle(),
        });
    };
    for name in dbus.list_names().await? {
        if is_player(&name) {
            add(&mut players, name.to_string());
        }
    }

    let mut shown = None;
    loop {
        let track = current(&mut players).await;
        if track != shown {
            out.send(track.clone()).await?;
            shown = track;
        }
        tokio::select! {
            signal = owners.next() => {
                let signal = signal.ok_or("the bus went away")?;
                let args = signal.args()?;
                let name = args.name.to_string();
                if !is_player(&name) {
                    continue;
                }
                // A new owner is a new player, even under the same name.
                players.retain(|p| p.name != name);
                if args.new_owner.is_some() {
                    add(&mut players, name);
                }
            }
            Some((name, generation, state)) = incoming.recv() => {
                if let Some(entry) =
                    players.iter_mut().find(|p| p.name == name && p.generation == generation)
                {
                    entry.state = Some(state);
                }
            }
        }
    }
}

fn is_player(name: &str) -> bool {
    // playerctld proxies the other players, so it would show each of them twice.
    name.starts_with(PREFIX) && !name.contains("playerctld")
}

/// The track to show, with its art fetched if it's new.
async fn current(players: &mut [Entry]) -> Option<Track> {
    let index = choose(players.iter().map(|p| p.state.as_ref()))?;
    let entry = &mut players[index];
    let state = entry.state.as_ref()?;
    let art = match &entry.art {
        Some((url, art)) if *url == state.art_url => art.clone(),
        _ => {
            let art = fetch_art(&state.art_url).await;
            entry.art = Some((state.art_url.clone(), art.clone()));
            art
        }
    };
    Some(state.track(&entry.name, art))
}

/// Which player to show, as NowPlaying.qml picks it: of those with a track title, the one
/// playing, else the first.
fn choose<'a>(players: impl Iterator<Item = Option<&'a Player>> + Clone) -> Option<usize> {
    let candidates = players
        .enumerate()
        .filter_map(|(i, p)| Some((i, p?)))
        .filter(|(_, p)| !p.title.is_empty());
    let first = candidates.clone().next()?.0;
    Some(
        candidates
            .clone()
            .find(|(_, p)| p.playing)
            .map_or(first, |(i, _)| i),
    )
}

/// Follows one player until its name goes away (when it's aborted) or it stops answering.
async fn follow(
    connection: zbus::Connection,
    name: String,
    generation: u64,
    reports: tokio::sync::mpsc::Sender<Report>,
) {
    // A player that fails here simply isn't shown, as in Quickshell.
    let _ = follow_player(&connection, &name, generation, &reports).await;
}

async fn follow_player(
    connection: &zbus::Connection,
    name: &str,
    generation: u64,
    reports: &tokio::sync::mpsc::Sender<Report>,
) -> Result {
    let properties = zbus::fdo::PropertiesProxy::builder(connection)
        .destination(name)?
        .path(PATH)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    // Uncached, so reading `Position` asks the player each time.
    let player = PlayerProxy::builder(connection)
        .destination(name)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let mut changes = properties.receive_properties_changed().await?;
    let mut seeked = player.receive_seeked().await?;
    let mut seeks = SEEKS.subscribe();
    let interface = InterfaceName::from_static_str_unchecked(INTERFACE);

    let mut state = Player::default();
    let all = properties.get_all(interface.clone()).await?;
    state.apply(all.iter().map(|(k, v)| (k.as_str(), &**v)), Instant::now());
    // When to read the position again, for players that settle late.
    let mut settle: Option<tokio::time::Instant> = None;
    loop {
        reports
            .send((name.to_string(), generation, state.clone()))
            .await?;
        tokio::select! {
            change = changes.next() => {
                let change = change.ok_or("player went away")?;
                let args = change.args()?;
                if args.interface_name != interface {
                    continue;
                }
                let changed = args.changed_properties.iter().map(|(k, v)| (*k, v));
                if state.apply(changed, Instant::now()) {
                    read_position(&player, &mut state).await;
                    settle = Some(tokio::time::Instant::now() + SETTLE);
                }
                if !args.invalidated_properties.is_empty() {
                    let all = properties.get_all(interface.clone()).await?;
                    state.apply(all.iter().map(|(k, v)| (k.as_str(), &**v)), Instant::now());
                }
            }
            signal = seeked.next() => {
                let signal = signal.ok_or("player went away")?;
                state.set_position(signal.args()?.position, Instant::now());
            }
            Ok(()) = seeks.changed() => {
                let seek = seeks.borrow_and_update().clone();
                if let Some(seek) = seek.filter(|s| s.player == name) {
                    // Take it straight away; the player confirms it shortly.
                    state.set_position(micros(seek.to), Instant::now());
                    settle = Some(tokio::time::Instant::now() + SETTLE);
                }
            }
            () = sleep_until(settle) => {
                settle = None;
                read_position(&player, &mut state).await;
            }
        }
    }
}

/// Never wakes when there's nothing to wait for.
async fn sleep_until(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

async fn read_position(player: &PlayerProxy<'_>, state: &mut Player) {
    if let Ok(position) = player.position().await {
        state.set_position(position, Instant::now());
    }
}

/// What we know of one player.
#[derive(Debug, Clone, PartialEq)]
struct Player {
    playing: bool,
    rate: f64,
    title: String,
    artist: String,
    art_url: String,
    track_id: Option<String>,
    /// `xesam:url`, which tells tracks apart for players without track ids.
    url: Option<String>,
    length: Option<Duration>,
    position: Option<Position>,
    can_control: bool,
    can_seek: bool,
    can_go_next: bool,
    can_go_previous: bool,
    can_play: bool,
    can_pause: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            playing: false,
            rate: 1.,
            title: String::new(),
            artist: String::new(),
            art_url: String::new(),
            track_id: None,
            url: None,
            length: None,
            position: None,
            can_control: true,
            can_seek: false,
            can_go_next: false,
            can_go_previous: false,
            can_play: false,
            can_pause: false,
        }
    }
}

impl Player {
    /// Takes in changed (or all) properties. True when the position should be read again:
    /// after play/pause, a new rate or a new track, as players don't always report a seek then.
    fn apply<'a, 'v: 'a>(
        &mut self,
        properties: impl IntoIterator<Item = (&'a str, &'a Value<'v>)>,
        now: Instant,
    ) -> bool {
        let mut reread = false;
        for (key, value) in properties {
            let value = inner(value);
            match key {
                "PlaybackStatus" => {
                    let playing = as_str(value) == Some("Playing");
                    if playing != self.playing {
                        self.playing = playing;
                        self.retime(now);
                        reread = true;
                    }
                }
                "Rate" => {
                    if let Some(rate) = as_f64(value).filter(|r| *r != self.rate) {
                        self.rate = rate;
                        self.retime(now);
                        reread = true;
                    }
                }
                "Metadata" => reread |= self.set_metadata(value),
                "Position" => {
                    if let Some(position) = as_i64(value) {
                        self.set_position(position, now);
                    }
                }
                "CanControl" => self.can_control = as_bool(value),
                "CanSeek" => self.can_seek = as_bool(value),
                "CanGoNext" => self.can_go_next = as_bool(value),
                "CanGoPrevious" => self.can_go_previous = as_bool(value),
                "CanPlay" => self.can_play = as_bool(value),
                "CanPause" => self.can_pause = as_bool(value),
                _ => {}
            }
        }
        reread
    }

    /// True when it's a different track.
    fn set_metadata(&mut self, metadata: &Value<'_>) -> bool {
        let mut fields: HashMap<&str, &Value<'_>> = HashMap::new();
        if let Value::Dict(dict) = metadata {
            for (key, value) in dict.iter() {
                if let Some(key) = as_str(inner(key)) {
                    fields.insert(key, inner(value));
                }
            }
        }
        let text = |key| fields.get(key).and_then(|v| as_str(v)).map(str::to_string);
        let before = (
            self.track_id.take(),
            self.url.take(),
            std::mem::take(&mut self.title),
        );
        self.track_id = text("mpris:trackid");
        self.url = text("xesam:url");
        self.title = text("xesam:title").unwrap_or_default();
        self.artist = match fields.get("xesam:artist") {
            Some(Value::Array(artists)) => artists
                .inner()
                .iter()
                .filter_map(|a| as_str(inner(a)))
                .collect::<Vec<_>>()
                .join(", "),
            Some(artist) => as_str(artist).unwrap_or_default().to_string(),
            None => String::new(),
        };
        self.art_url = text("mpris:artUrl").unwrap_or_default();
        self.length = fields
            .get("mpris:length")
            .and_then(|v| as_i64(v))
            .filter(|l| *l > 0)
            .map(|l| Duration::from_micros(l as u64));
        // Some players (Jellyfin) keep the id or URL across tracks, so the title counts too.
        before != (self.track_id.clone(), self.url.clone(), self.title.clone())
    }

    fn set_position(&mut self, micros: i64, now: Instant) {
        let reported = Duration::from_micros(micros.max(0) as u64);
        self.position = Some(Position::new(reported, now, self.rate, self.playing));
    }

    /// Counts a new rate or play state from `now` on.
    fn retime(&mut self, now: Instant) {
        if let Some(position) = &mut self.position {
            position.retime(now, self.rate, self.playing);
        }
    }

    fn track(&self, player: &str, art: Option<icons::Image>) -> Track {
        // As in Quickshell, a player that can't be controlled can do none of it.
        let can = |flag: bool| self.can_control && flag;
        Track {
            player: player.to_string(),
            playing: self.playing,
            title: self.title.clone(),
            artist: self.artist.clone(),
            art,
            length: self.length,
            position: self.position,
            track_id: self.track_id.clone(),
            can_seek: can(self.can_seek),
            can_go_next: can(self.can_go_next),
            can_go_previous: can(self.can_go_previous),
            can_play: can(self.can_play),
            can_pause: can(self.can_pause),
        }
    }
}

/// Unwraps variants in variants.
fn inner<'a, 'v>(value: &'a Value<'v>) -> &'a Value<'v> {
    match value {
        Value::Value(boxed) => inner(boxed),
        value => value,
    }
}

fn as_str<'a>(value: &'a Value<'_>) -> Option<&'a str> {
    match value {
        Value::Str(s) => Some(s.as_str()),
        Value::ObjectPath(p) => Some(p.as_str()),
        _ => None,
    }
}

/// Integers of any width: players disagree on `mpris:length`'s type.
fn as_i64(value: &Value<'_>) -> Option<i64> {
    match *value {
        Value::U8(n) => Some(n.into()),
        Value::I16(n) => Some(n.into()),
        Value::U16(n) => Some(n.into()),
        Value::I32(n) => Some(n.into()),
        Value::U32(n) => Some(n.into()),
        Value::I64(n) => Some(n),
        Value::U64(n) => i64::try_from(n).ok(),
        Value::F64(n) if n.is_finite() => Some(n as i64),
        _ => None,
    }
}

fn as_f64(value: &Value<'_>) -> Option<f64> {
    match *value {
        Value::F64(n) => Some(n),
        _ => as_i64(value).map(|n| n as f64),
    }
}

fn as_bool(value: &Value<'_>) -> bool {
    matches!(value, Value::Bool(true))
}

fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

async fn fetch_art(url: &str) -> Option<icons::Image> {
    if let Some(path) = url.strip_prefix("file://") {
        return icons::load(std::path::Path::new(path));
    }
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return None;
    }
    let url = url.to_string();
    let bytes = tokio::task::spawn_blocking(move || {
        let mut response = ureq::get(&url).call().ok()?;
        response
            .body_mut()
            .with_config()
            .limit(8 << 20)
            .read_to_vec()
            .ok()
    })
    .await
    .ok()??;
    icons::decode(&bytes).map(icons::Image::Raster)
}

/// The session bus for the controls, connected once.
async fn bus() -> zbus::Result<zbus::Connection> {
    static BUS: tokio::sync::OnceCell<zbus::Connection> = tokio::sync::OnceCell::const_new();
    BUS.get_or_try_init(zbus::Connection::session)
        .await
        .cloned()
}

/// Runs `call` on `player`'s Player interface, logging a failure.
async fn control<F>(player: String, call: impl FnOnce(PlayerProxy<'static>) -> F)
where
    F: Future<Output = zbus::Result<()>>,
{
    let proxy = async {
        PlayerProxy::builder(&bus().await?)
            .destination(player.clone())?
            .cache_properties(CacheProperties::No)
            .build()
            .await
    };
    let result = match proxy.await {
        Ok(proxy) => call(proxy).await,
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        eprintln!("media: {player}: {e}");
    }
}

pub async fn play_pause(player: String) {
    control(player, |p| async move { p.play_pause().await }).await;
}

pub async fn next(player: String) {
    control(player, |p| async move { p.next().await }).await;
}

pub async fn previous(player: String) {
    control(player, |p| async move { p.previous().await }).await;
}

/// Moves playback in `track` to `to`, if its player can seek. `SetPosition` needs the track id;
/// without one it's a relative `Seek` from where playback is estimated to be.
pub fn seek(track: &Track, to: Duration) -> impl Future<Output = ()> + Send + 'static {
    let can_seek = track.can_seek;
    let to = track.length.map_or(to, |length| to.min(length));
    let player = track.player.clone();
    let track_id = track
        .track_id
        .clone()
        .filter(|id| id != NO_TRACK && ObjectPath::try_from(id.as_str()).is_ok());
    let offset = micros(to) - micros(track.position_at(Instant::now()).unwrap_or_default());
    async move {
        if !can_seek {
            return;
        }
        let seek = Seek {
            player: player.clone(),
            to,
        };
        control(player, move |p| async move {
            match track_id {
                Some(id) => {
                    let id = ObjectPath::try_from(id.as_str())?;
                    p.set_position(&id, micros(to)).await
                }
                None => p.seek(offset).await,
            }
        })
        .await;
        SEEKS.send_replace(Some(seek));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    fn close(a: Duration, b: Duration) -> bool {
        (a.as_secs_f64() - b.as_secs_f64()).abs() < 1e-6
    }

    #[test]
    fn position_moves_only_while_playing() {
        let t0 = Instant::now();
        let playing = Position::new(secs(10.), t0, 1., true);
        assert!(close(playing.at(t0 + secs(2.5)), secs(12.5)));
        let paused = Position::new(secs(10.), t0, 1., false);
        assert_eq!(paused.at(t0 + secs(2.5)), secs(10.));
    }

    #[test]
    fn position_follows_rate() {
        let t0 = Instant::now();
        let at = |rate| Position::new(secs(5.), t0, rate, true).at(t0 + secs(3.));
        assert!(close(at(2.), secs(11.)));
        assert!(close(at(0.5), secs(6.5)));
        assert_eq!(at(0.), secs(5.));
        assert_eq!(at(f64::NAN), secs(5.));
    }

    #[test]
    fn ticks_on_the_next_second() {
        let t0 = Instant::now();
        let p = Position::new(secs(10.25), t0, 1., true);
        assert!(close(p.until_next_second(t0).unwrap(), secs(0.76)));
        let fast = Position::new(secs(10.), t0, 2., true);
        assert!(close(fast.until_next_second(t0).unwrap(), secs(0.51)));
        let slow = Position::new(secs(10.), t0, 0.1, true);
        assert!(close(slow.until_next_second(t0).unwrap(), secs(1.01)));
        let paused = Position::new(secs(1.), t0, 1., false);
        assert_eq!(paused.until_next_second(t0), None);
    }

    fn props<'a>(
        pairs: &'a [(&'a str, Value<'a>)],
    ) -> impl Iterator<Item = (&'a str, &'a Value<'a>)> {
        pairs.iter().map(|(k, v)| (*k, v))
    }

    fn status(s: &str) -> [(&str, Value<'_>); 1] {
        [("PlaybackStatus", Value::from(s))]
    }

    #[test]
    fn pausing_freezes_the_estimate_where_it_was() {
        let t0 = Instant::now();
        let mut player = Player::default();
        player.apply(props(&status("Playing")), t0);
        player.set_position(5_000_000, t0);
        assert!(player.apply(props(&status("Paused")), t0 + secs(2.)));
        assert!(close(player.position.unwrap().at(t0 + secs(60.)), secs(7.)));
        // Resuming counts on from the pause.
        assert!(player.apply(props(&status("Playing")), t0 + secs(30.)));
        assert!(close(player.position.unwrap().at(t0 + secs(31.)), secs(8.)));
        // The same status again isn't a change.
        assert!(!player.apply(props(&status("Playing")), t0 + secs(31.)));
    }

    #[test]
    fn rate_change_counts_from_when_it_happened() {
        let t0 = Instant::now();
        let mut player = Player::default();
        player.apply(props(&status("Playing")), t0);
        player.set_position(0, t0);
        assert!(player.apply(props(&[("Rate", Value::from(2.))]), t0 + secs(4.)));
        assert!(close(player.position.unwrap().at(t0 + secs(5.)), secs(6.)));
        assert!(!player.apply(props(&[("Rate", Value::from(2.))]), t0 + secs(5.)));
    }

    #[test]
    fn seeking_is_assumed_until_the_player_says() {
        let t0 = Instant::now();
        let mut player = player("x", true);
        player.length = Some(secs(100.));
        player.set_position(1_000_000, t0);
        let mut track = player.track("org.mpris.MediaPlayer2.test", None);
        track.assume_seeked(secs(50.), t0 + secs(1.));
        assert!(close(track.position_at(t0 + secs(3.)).unwrap(), secs(52.)));
    }

    fn metadata(title: &str, track_id: &str, length: Value<'static>) -> Value<'static> {
        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("xesam:title".into(), Value::from(title.to_string()));
        map.insert(
            "mpris:trackid".into(),
            Value::from(ObjectPath::try_from(track_id.to_string()).unwrap()),
        );
        map.insert("mpris:length".into(), length);
        map.insert(
            "xesam:artist".into(),
            Value::from(vec!["A".to_string(), "B".to_string()]),
        );
        Value::from(map)
    }

    #[test]
    fn metadata_and_track_changes() {
        let t0 = Instant::now();
        let mut player = Player::default();
        let one = metadata("One", "/t/1", Value::from(180_000_000i64));
        assert!(player.apply(props(&[("Metadata", one.try_clone().unwrap())]), t0));
        assert_eq!(player.title, "One");
        assert_eq!(player.artist, "A, B");
        assert_eq!(player.track_id.as_deref(), Some("/t/1"));
        assert_eq!(player.length, Some(secs(180.)));
        // The same track again is no change; another one is.
        assert!(!player.apply(props(&[("Metadata", one)]), t0));
        let two = metadata("Two", "/t/2", Value::from(90_000_000u64));
        assert!(player.apply(props(&[("Metadata", two)]), t0));
        assert_eq!(player.length, Some(secs(90.)));
    }

    #[test]
    fn position_is_kept_within_the_track() {
        let t0 = Instant::now();
        let mut player = player("x", true);
        player.length = Some(secs(10.));
        player.set_position(9_000_000, t0);
        let track = player.track("org.mpris.MediaPlayer2.test", None);
        assert_eq!(track.position_at(t0 + secs(5.)), Some(secs(10.)));
        assert!(track.has_progress());
    }

    fn player(title: &str, playing: bool) -> Player {
        Player {
            title: title.into(),
            playing,
            ..Player::default()
        }
    }

    #[test]
    fn chooses_like_now_playing_qml() {
        let paused = player("a", false);
        let playing = player("b", true);
        let untitled = player("", true);
        // The one playing, even when it isn't first.
        assert_eq!(choose([Some(&paused), Some(&playing)].into_iter()), Some(1));
        // Else the first with a title; untitled ones and ones not heard from yet don't count.
        let players = [None, Some(&untitled), Some(&paused), Some(&paused)];
        assert_eq!(choose(players.into_iter()), Some(2));
        assert_eq!(choose([Some(&untitled), None].into_iter()), None);
        assert!(!is_player("org.mpris.MediaPlayer2.playerctld"));
        assert!(is_player("org.mpris.MediaPlayer2.spotify"));
        assert!(!is_player("org.freedesktop.Notifications"));
    }
}
