//! Forwarded addresses are only accepted from explicitly trusted TCP peers.
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientIpHeader {
    #[default]
    XForwardedFor,
    XRealIp,
    CfConnectingIp,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProxyConfig {
    pub trusted_proxies: Vec<String>,
    pub client_ip_header: ClientIpHeader,
}
impl ProxyConfig {
    pub fn validate(&self) -> bool {
        self.trusted_proxies.len() <= 128
            && self
                .trusted_proxies
                .iter()
                .all(|value| network(value).is_some())
    }
    pub fn trusts(&self, address: IpAddr) -> bool {
        self.trusted_proxies.iter().any(|value| {
            network(value).is_some_and(|(base, prefix)| contains(base, prefix, address))
        })
    }
    pub fn resolve(&self, peer: Option<IpAddr>, headers: &HeaderMap) -> Option<IpAddr> {
        let peer = peer?;
        if !self.trusts(peer) {
            return Some(peer);
        }
        let name = match self.client_ip_header {
            ClientIpHeader::XForwardedFor => "x-forwarded-for",
            ClientIpHeader::XRealIp => "x-real-ip",
            ClientIpHeader::CfConnectingIp => "cf-connecting-ip",
        };
        let values: Vec<_> = headers.get_all(name).iter().collect();
        if values.is_empty() || values.iter().map(|v| v.as_bytes().len()).sum::<usize>() > 8192 {
            return Some(peer);
        }
        if self.client_ip_header != ClientIpHeader::XForwardedFor {
            if values.len() != 1 {
                return Some(peer);
            }
            return values[0]
                .to_str()
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .or(Some(peer));
        }
        let mut chain = Vec::new();
        for value in values {
            let Ok(value) = value.to_str() else {
                return Some(peer);
            };
            for part in value.split(',') {
                let Ok(address) = part.trim().parse::<IpAddr>() else {
                    return Some(peer);
                };
                chain.push(address);
                if chain.len() > 32 {
                    return Some(peer);
                }
            }
        }
        // Stop at the first untrusted hop, never at an attacker-controlled leftmost value.
        for address in chain.iter().rev() {
            if !self.trusts(*address) {
                return Some(*address);
            }
        }
        chain.first().copied().or(Some(peer))
    }
}
fn network(value: &str) -> Option<(IpAddr, u8)> {
    let (address, prefix) = match value.split_once('/') {
        Some((address, prefix)) => (
            address.parse::<IpAddr>().ok()?,
            Some(prefix.parse::<u8>().ok()?),
        ),
        None => (value.parse::<IpAddr>().ok()?, None),
    };
    let width = if address.is_ipv4() { 32 } else { 128 };
    let prefix = prefix.unwrap_or(width);
    (prefix <= width).then_some((address, prefix))
}
fn contains(base: IpAddr, prefix: u8, address: IpAddr) -> bool {
    let address = match address {
        IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(address),
        v => v,
    };
    match (base, address) {
        (IpAddr::V4(base), IpAddr::V4(address)) => {
            let mask = u32::MAX.checked_shl(32 - prefix as u32).unwrap_or(0);
            u32::from(base) & mask == u32::from(address) & mask
        }
        (IpAddr::V6(base), IpAddr::V6(address)) => {
            let mask = u128::MAX.checked_shl(128 - prefix as u32).unwrap_or(0);
            u128::from(base) & mask == u128::from(address) & mask
        }
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn peer(v: &str) -> Option<IpAddr> {
        Some(v.parse().unwrap())
    }
    #[test]
    fn direct_requests_and_unknown_peers_cannot_spoof_headers() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());
        let c = ProxyConfig::default();
        assert_eq!(c.resolve(peer("192.0.2.1"), &h), peer("192.0.2.1"));
        assert_eq!(c.resolve(None, &h), None);
    }
    #[test]
    fn forwarded_chain_stops_at_untrusted_hop_and_checks_all_tokens() {
        let c = ProxyConfig {
            trusted_proxies: vec!["127.0.0.1".into(), "10.0.0.0/8".into()],
            ..Default::default()
        };
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            "6.6.6.6, 198.51.100.42, 10.1.2.3".parse().unwrap(),
        );
        assert_eq!(c.resolve(peer("127.0.0.1"), &h), peer("198.51.100.42"));
        h.insert("x-forwarded-for", "garbage, 198.51.100.42".parse().unwrap());
        assert_eq!(c.resolve(peer("127.0.0.1"), &h), peer("127.0.0.1"));
        h.insert("x-forwarded-for", "198.51.100.42".parse().unwrap());
        assert_eq!(c.resolve(peer("203.0.113.3"), &h), peer("203.0.113.3"));
    }
    #[test]
    fn single_headers_ipv6_and_invalid_ranges_are_handled() {
        let c = ProxyConfig {
            trusted_proxies: vec!["::1/128".into()],
            client_ip_header: ClientIpHeader::CfConnectingIp,
        };
        let mut h = HeaderMap::new();
        h.insert("cf-connecting-ip", "2001:db8::42".parse().unwrap());
        assert_eq!(c.resolve(peer("::1"), &h), peer("2001:db8::42"));
        h.append("cf-connecting-ip", "198.51.100.1".parse().unwrap());
        assert_eq!(c.resolve(peer("::1"), &h), peer("::1"));
        for bad in ["0.0.0.0/33", "::/129", "localhost", "127.0.0.1/-1"] {
            assert!(!ProxyConfig {
                trusted_proxies: vec![bad.into()],
                ..Default::default()
            }
            .validate());
        }
    }
}
