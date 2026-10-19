//! Taildrop targets: your own devices on the tailnet (Taildrop doesn't send to others').

use serde::{Deserialize, Serialize};

use crate::tools;

#[derive(Serialize, Debug)]
pub struct Peer {
    /// What `tailscale file cp` takes before the colon.
    pub name: String,
    pub online: bool,
    pub os: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Status {
    #[serde(rename = "Self")]
    me: Option<Node>,
    #[serde(default)]
    peer: std::collections::HashMap<String, Node>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Node {
    #[serde(default, rename = "DNSName")]
    dns_name: String,
    #[serde(default)]
    host_name: String,
    #[serde(default, rename = "OS")]
    os: String,
    #[serde(default)]
    online: bool,
    #[serde(default, rename = "UserID")]
    user_id: u64,
}

pub async fn list() -> anyhow::Result<Vec<Peer>> {
    let out = tools::output("tailscale", ["status", "--json"]).await?;
    parse(&out)
}

fn parse(json: &str) -> anyhow::Result<Vec<Peer>> {
    let status: Status = serde_json::from_str(json)?;
    let me = status.me.map(|m| m.user_id);
    let mut peers: Vec<Peer> = status
        .peer
        .into_values()
        .filter(|p| me.is_none_or(|id| id == p.user_id))
        .filter(|p| !p.host_name.starts_with("funnel-ingress-node"))
        .map(|p| Peer {
            // The MagicDNS label is what's unique; HostName can repeat.
            name: p
                .dns_name
                .split('.')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(&p.host_name)
                .to_string(),
            online: p.online,
            os: p.os,
        })
        .collect();
    peers.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.name.cmp(&b.name)));
    Ok(peers)
}

#[cfg(test)]
mod tests {
    #[test]
    fn own_devices_online_first() {
        let json = r#"{
            "Self": {"DNSName": "xps.tail.ts.net.", "HostName": "xps", "UserID": 1, "Online": true},
            "Peer": {
                "a": {"DNSName": "pi.tail.ts.net.", "HostName": "raspberrypi", "UserID": 1, "Online": false, "OS": "linux"},
                "b": {"DNSName": "desktop.tail.ts.net.", "HostName": "desktop", "UserID": 1, "Online": true, "OS": "linux"},
                "c": {"DNSName": "friend.tail.ts.net.", "HostName": "friend", "UserID": 2, "Online": true}
            }
        }"#;
        let names: Vec<_> = super::parse(json)
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["desktop", "pi"]);
    }
}
