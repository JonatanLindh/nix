//! Sound devices, natively from PipeWire through `wirehose` (wiremix's PipeWire layer, ported
//! into `services::wirehose`), the way Quickshell does it:
//!
//! - A node backed by a sound card takes its volume and mute from the card's active route (the
//!   route whose device is the node's `card.profile.device`), and changes go there too; other
//!   nodes (EasyEffects, virtual sinks) are changed on the node itself.
//! - Volumes are cubic on the wire: shown as the cube root, written cubed.
//! - One route change is in flight per device at a time; the latest value waits for the device
//!   to report back, so a dragged slider sends what the hardware can take, not every step.
//! - Defaults are read from `default.audio.*` in the "default" metadata and set through
//!   `default.configured.audio.*`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::wirehose::state::{Device as PwDevice, Node, Route as PwRoute, State as Graph};
use super::wirehose::{Event, ObjectId, Session, StateEvent};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    /// Hardware outputs, for the output picker.
    pub sinks: Vec<Device>,
    /// Hardware inputs, for the input picker.
    pub sources: Vec<Device>,
    /// The device sound ends up on; volume always applies here.
    pub output: Option<Device>,
    pub input: Option<Device>,
    /// Virtual sinks in front of the output (EasyEffects, filter chains), by label.
    pub filters: Vec<String>,
    /// EasyEffects' sink exists, so it's running.
    pub easyeffects: bool,
    /// Apps recording audio right now (for the privacy indicators).
    pub capturing: Vec<String>,
}

/// A device, by its PipeWire node id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: DeviceId,
    pub label: String,
    /// 0–1 as shown (the cube root of PipeWire's volume).
    pub volume: f32,
    pub muted: bool,
    /// What it is, as one of our icons.
    pub icon: &'static str,
}

impl State {
    pub fn muted(&self) -> bool {
        self.output.as_ref().is_some_and(|d| d.muted)
    }

    pub fn percent(&self) -> u32 {
        self.output
            .as_ref()
            .map_or(0, |d| (d.volume * 100.).round() as u32)
    }

    /// The bar's icon.
    pub fn icon(&self) -> &'static str {
        match self.percent() {
            _ if self.muted() => "volume-mute",
            0 => "volume-mute",
            1..50 => "volume-low",
            _ => "volume",
        }
    }
}

/// A change for the sound server.
#[derive(Debug, Clone)]
pub enum Command {
    /// 0–1 as shown; clamped. Also unmutes, like Audio.qml's setVolume.
    Volume(Device, f32),
    Mute(Device, bool),
    /// Makes it the default output or input.
    Default(Device),
}

/// What sending commands needs: the session, the graph as last seen, and the pacing.
struct Shared {
    session: Session,
    graph: Graph,
    pacer: Pacer,
}

static SHARED: Mutex<Option<Arc<Mutex<Shared>>>> = Mutex::new(None);

pub fn watch() -> UnboundedReceiver<State> {
    let (tx, rx) = unbounded();
    std::thread::Builder::new()
        .name("pipewire-watch".into())
        .spawn(move || run(tx))
        .expect("pipewire thread");
    rx
}

/// Keeps a session up for as long as anyone listens, reconnecting after PipeWire restarts. The
/// session ends itself when the connection goes (an error about the core); errors about single
/// objects, a Bluetooth node failing say, are only logged.
fn run(tx: UnboundedSender<State>) {
    let mut last_error = String::new();
    while !tx.is_closed() {
        let (events_tx, events) = std::sync::mpsc::channel();
        let session = match Session::spawn(move |event| events_tx.send(event).is_ok()) {
            Ok(session) => session,
            Err(e) => {
                log_once(&mut last_error, e.to_string());
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };
        let shared = Arc::new(Mutex::new(Shared {
            session,
            graph: Graph::default(),
            pacer: Pacer::default(),
        }));
        *SHARED.lock().unwrap() = Some(shared.clone());

        let mut ready = false;
        let mut last = None;
        for event in events.iter() {
            match event {
                Event::State(event) => {
                    let mut s = shared.lock().unwrap();
                    let reported = match &event {
                        StateEvent::DeviceRoute { object_id, .. } => Some(*object_id),
                        _ => None,
                    };
                    s.graph.update(event);
                    // A device reported its route: the change in flight there is done.
                    if let Some(device) = reported {
                        let Shared {
                            session,
                            graph,
                            pacer,
                        } = &mut *s;
                        pacer.confirmed(device, session, graph);
                    }
                }
                Event::Ready => ready = true,
                Event::Error { object_id, message } => {
                    let about = object_id.map_or("setup".into(), |id| id.to_string());
                    log_once(&mut last_error, format!("{message} ({about})"));
                }
            }
            if !ready {
                continue;
            }
            let state = snapshot(&shared.lock().unwrap().graph);
            if last.as_ref() != Some(&state) {
                last = Some(state.clone());
                if tx.unbounded_send(state).is_err() {
                    return;
                }
            }
        }
        // The session's thread has ended: its events are all in.
        *SHARED.lock().unwrap() = None;
        // Nothing is known about the devices until the next session reports.
        if tx.unbounded_send(State::default()).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn log_once(last: &mut String, error: String) {
    if *last != error {
        eprintln!("audio: {error}");
        *last = error;
    }
}

/// Sends `command` to PipeWire. Dropped while it isn't connected.
pub fn send(command: Command) {
    let Some(shared) = SHARED.lock().unwrap().clone() else {
        return;
    };
    let mut s = shared.lock().unwrap();
    let Shared {
        session,
        graph,
        pacer,
    } = &mut *s;
    let node_id = |d: &Device| ObjectId::from_raw_id(d.id.0);
    match command {
        Command::Volume(device, value) => {
            pacer.volume(node_id(&device), value.clamp(0., 1.), session, graph);
        }
        Command::Mute(device, mute) => {
            let id = node_id(&device);
            match route_of(graph, id) {
                Some(r) => session.device_mute(r.device, r.index, r.card_device, mute),
                None => session.node_mute(id, mute),
            }
        }
        Command::Default(device) => {
            let id = node_id(&device);
            let (Some(node), Some(metadata)) =
                (graph.nodes.get(&id), graph.get_metadata_by_name("default"))
            else {
                return;
            };
            let Some(name) = node.props.node_name() else {
                return;
            };
            let key = if is_source(node) {
                "default.configured.audio.source"
            } else {
                "default.configured.audio.sink"
            };
            session.metadata_set_property(
                metadata.object_id,
                0,
                key.into(),
                Some("Spa:String:JSON".into()),
                Some(serde_json::json!({ "name": name }).to_string()),
            );
        }
    }
}

/// Where a card-backed node's volume lives: its device's active route.
#[derive(Debug, Clone, Copy)]
struct Route {
    device: ObjectId,
    index: i32,
    card_device: i32,
}

fn route_of(graph: &Graph, node: ObjectId) -> Option<Route> {
    let node = graph.nodes.get(&node)?;
    let device = *node.props.device_id()?;
    let card_device = *node.props.card_profile_device()?;
    let route = active_route(graph.devices.get(&device)?, card_device)?;
    Some(Route {
        device,
        index: route.index,
        card_device,
    })
}

fn active_route(device: &PwDevice, card_device: i32) -> Option<&PwRoute> {
    device.routes.values().find(|r| r.device == card_device)
}

/// Volume changes, paced per device: while one is in flight, only the latest waits.
#[derive(Default)]
struct Pacer {
    /// Devices with a route change sent and not yet reported back, and since when.
    in_flight: HashMap<ObjectId, Instant>,
    /// The latest volume asked for while one was in flight, per device: (node, value).
    waiting: HashMap<ObjectId, (ObjectId, f32)>,
}

/// A device that never reports back mustn't hold volume changes forever.
const CONFIRM_TIMEOUT: Duration = Duration::from_millis(250);

impl Pacer {
    fn volume(&mut self, node: ObjectId, value: f32, session: &Session, graph: &Graph) {
        let Some(route) = route_of(graph, node) else {
            // Not card-backed: set on the node, with no route to wait on.
            let channels = graph
                .nodes
                .get(&node)
                .and_then(|n| n.volumes.as_ref())
                .map_or(2, Vec::len);
            session.node_mute(node, false);
            session.node_volumes(node, vec![value.powi(3); channels.max(1)]);
            return;
        };
        let busy = self
            .in_flight
            .get(&route.device)
            .is_some_and(|at| at.elapsed() < CONFIRM_TIMEOUT);
        if busy {
            self.waiting.insert(route.device, (node, value));
        } else {
            self.send(value, route, session, graph);
        }
    }

    fn send(&mut self, value: f32, route: Route, session: &Session, graph: &Graph) {
        let channels = graph
            .devices
            .get(&route.device)
            .and_then(|d| active_route(d, route.card_device))
            .map_or(2, |r| r.volumes.len());
        session.device_mute(route.device, route.index, route.card_device, false);
        session.device_volumes(
            route.device,
            route.index,
            route.card_device,
            vec![value.powi(3); channels.max(1)],
        );
        self.in_flight.insert(route.device, Instant::now());
    }

    fn confirmed(&mut self, device: ObjectId, session: &Session, graph: &Graph) {
        self.in_flight.remove(&device);
        if let Some((node, value)) = self.waiting.remove(&device)
            && let Some(route) = route_of(graph, node)
        {
            self.send(value, route, session, graph);
        }
    }
}

fn media_class(node: &Node) -> &str {
    node.props.media_class().map_or("", String::as_str)
}

fn is_source(node: &Node) -> bool {
    media_class(node).starts_with("Audio/Source")
}

fn is_hardware(node: &Node) -> bool {
    node.props.device_id().is_some()
}

/// The node named in the "default" metadata under `key`.
fn default_name(graph: &Graph, key: &str) -> Option<String> {
    let metadata = graph.get_metadata_by_name("default")?;
    let json = metadata.properties.get(&0)?.get(key)?;
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    Some(value["name"].as_str()?.to_string())
}

fn device(graph: &Graph, id: ObjectId, node: &Node) -> Option<Device> {
    let route = route_of(graph, id).and_then(|r| {
        graph
            .devices
            .get(&r.device)
            .and_then(|d| active_route(d, r.card_device))
    });
    let (volumes, muted) = match route {
        Some(route) => (route.volumes.clone(), route.mute),
        None => (node.volumes.clone()?, node.mute?),
    };
    let linear = volumes.iter().sum::<f32>() / volumes.len().max(1) as f32;
    let label = ["node.description", "node.nick", "node.name"]
        .into_iter()
        .filter_map(|k| node.props.raw(k))
        .find(|s| !s.is_empty())
        .unwrap_or("Unknown")
        .to_string();
    Some(Device {
        id: DeviceId(id.into()),
        icon: device_icon(node, &label),
        label,
        volume: linear.max(0.).cbrt(),
        muted,
    })
}

/// What a device is, from its properties, as one of our icons.
fn device_icon(node: &Node, label: &str) -> &'static str {
    let s = ["device.form-factor", "device.icon-name", "node.name"]
        .iter()
        .filter_map(|k| node.props.raw(k))
        .chain([label])
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| s.contains(w));
    if has(&["headset", "headphone", "handsfree", "hands-free", "bluez"]) {
        "headphones"
    } else if is_source(node) {
        if has(&["webcam", "camera", "video"]) {
            "camera"
        } else {
            "mic"
        }
    } else if has(&["hdmi", "displayport", "monitor", "tv"]) {
        "monitor"
    } else {
        "speaker"
    }
}

/// The hardware sink `start` ends up feeding, following links.
fn downstream(graph: &Graph, start: ObjectId) -> Option<ObjectId> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        for next in graph.outputs(id) {
            let Some(node) = graph.nodes.get(&next) else {
                continue;
            };
            if is_hardware(node) && media_class(node) == "Audio/Sink" {
                return Some(next);
            }
            queue.push_back(next);
        }
    }
    None
}

fn snapshot(graph: &Graph) -> State {
    let mut sinks = Vec::new();
    let mut sources = Vec::new();
    let mut virtual_sinks = Vec::new();
    let mut capturing = Vec::new();
    let mut by_name = HashMap::new();
    for (&id, node) in &graph.nodes {
        if let Some(name) = node.props.node_name() {
            by_name.insert(name.clone(), id);
        }
        match media_class(node) {
            "Audio/Sink" if is_hardware(node) => sinks.extend(device(graph, id, node)),
            "Audio/Sink" => virtual_sinks.push(id),
            "Audio/Source" if is_hardware(node) => sources.extend(device(graph, id, node)),
            // A recording stream that's linked to something is recording.
            "Stream/Input/Audio" if !graph.inputs(id).is_empty() => {
                let app = node
                    .props
                    .raw("application.name")
                    .or(node.props.raw("node.name"));
                capturing.extend(app.map(str::to_string));
            }
            _ => {}
        }
    }
    sinks.sort_by(|a, b| a.label.cmp(&b.label));
    sources.sort_by(|a, b| a.label.cmp(&b.label));
    capturing.sort();
    capturing.dedup();

    let lookup = |key| default_name(graph, key).and_then(|n| by_name.get(&n).copied());
    let find =
        |list: &[Device], id: ObjectId| list.iter().find(|d| d.id.0 == u32::from(id)).cloned();

    // Sound ends up on a hardware sink: a virtual default (EasyEffects) feeds one downstream.
    let output_id = lookup("default.audio.sink").and_then(|id| {
        let node = graph.nodes.get(&id)?;
        if is_hardware(node) {
            Some(id)
        } else {
            downstream(graph, id)
        }
    });
    let output = output_id
        .and_then(|id| find(&sinks, id))
        .or_else(|| sinks.first().cloned());
    let input = lookup("default.audio.source")
        .and_then(|id| find(&sources, id))
        .or_else(|| sources.first().cloned());

    // Virtual sinks feeding the output. EasyEffects unlinks its chain while nothing plays, so an
    // unlinked one is taken to feed the current device.
    let mut filters: Vec<(String, String)> = virtual_sinks
        .iter()
        .filter_map(|&id| {
            let node = graph.nodes.get(&id)?;
            let target = downstream(graph, id);
            if target.is_some() && target != output_id {
                return None;
            }
            let name = node.props.node_name().cloned().unwrap_or_default();
            let label = if name == "easyeffects_sink" {
                "EasyEffects".to_string()
            } else {
                node.props
                    .raw("node.description")
                    .unwrap_or(&name)
                    .trim_end_matches(" sink")
                    .trim_end_matches(" Sink")
                    .to_string()
            };
            Some((name, label))
        })
        .collect();
    filters.sort();

    State {
        sinks,
        sources,
        output,
        input,
        easyeffects: by_name.contains_key("easyeffects_sink"),
        filters: filters.into_iter().map(|(_, label)| label).collect(),
        capturing,
    }
}

/// Sound on the UI side: the server's state as last reported, plus the changes made here that
/// the server hasn't caught up with.
pub struct Audio {
    /// `None` until PipeWire reports (or while there is none).
    pub state: Option<State>,
    /// The device whose volume was just changed here, and when: the server's reports of the
    /// steps before it arrive afterwards, and mustn't move the slider back.
    hold: Option<(DeviceId, Instant)>,
}

impl Audio {
    pub fn new(cx: &mut gpui::App) -> gpui::Entity<Self> {
        use futures::StreamExt;
        use gpui::AppContext;
        cx.new(|cx: &mut gpui::Context<Self>| {
            cx.spawn(async move |this, cx| {
                let mut reports = watch();
                while let Some(state) = reports.next().await {
                    let alive = this.update(cx, |audio, cx| {
                        audio.reported(state);
                        cx.notify();
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
            Self {
                state: None,
                hold: None,
            }
        })
    }

    /// The device sound goes to.
    pub fn output(&self) -> Option<&Device> {
        self.state.as_ref()?.output.as_ref()
    }

    /// Shows the new volume (0–1) right away and sends it on; the slider doesn't wait for the
    /// server.
    pub fn set_volume(&mut self, id: DeviceId, value: f32, cx: &mut gpui::Context<Self>) {
        let mut sent = None;
        for device in self.devices_mut(id) {
            device.volume = value;
            device.muted = false;
            sent.get_or_insert_with(|| device.clone());
        }
        if let Some(device) = sent {
            self.hold = Some((id, Instant::now()));
            send(Command::Volume(device, value));
            cx.notify();
        }
    }

    /// The output's volume, two percent up or down.
    pub fn nudge(&mut self, up: bool, cx: &mut gpui::Context<Self>) {
        if let Some(output) = self.output() {
            let step = if up { 0.02 } else { -0.02 };
            let (id, value) = (output.id, (output.volume + step).clamp(0., 1.));
            self.set_volume(id, value, cx);
        }
    }

    pub fn toggle_mute(&mut self, id: DeviceId, cx: &mut gpui::Context<Self>) {
        let mut sent = None;
        for device in self.devices_mut(id) {
            device.muted = !device.muted;
            sent.get_or_insert_with(|| device.clone());
        }
        if let Some(device) = sent {
            let muted = device.muted;
            send(Command::Mute(device, muted));
            cx.notify();
        }
    }

    /// Makes `id` the default output or input.
    pub fn set_default(&mut self, id: DeviceId) {
        if let Some(device) = self.devices_mut(id).next() {
            send(Command::Default(device.clone()));
        }
    }

    /// Every copy of the device `id` in the state (the lists, and output or input).
    fn devices_mut(&mut self, id: DeviceId) -> impl Iterator<Item = &mut Device> {
        self.state.iter_mut().flat_map(move |s| s.devices_mut(id))
    }

    /// A report from the sound server. For a moment after a local change, the device keeps the
    /// value set here: reports of the earlier steps of a drag arrive late.
    fn reported(&mut self, mut state: State) {
        const HOLD: Duration = Duration::from_millis(400);
        if let Some((id, at)) = self.hold
            && at.elapsed() < HOLD
            && let Some(local) = self.devices_mut(id).next().map(|d| (d.volume, d.muted))
        {
            for device in state.devices_mut(id) {
                (device.volume, device.muted) = local;
            }
        }
        self.state = Some(state);
    }
}

impl State {
    fn devices_mut(&mut self, id: DeviceId) -> impl Iterator<Item = &mut Device> {
        self.sinks
            .iter_mut()
            .chain(self.sources.iter_mut())
            .chain(self.output.iter_mut())
            .chain(self.input.iter_mut())
            .filter(move |d| d.id == id)
    }
}
