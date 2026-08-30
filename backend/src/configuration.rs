use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Configuration {
    pub bind_address: String,
    pub database_url: String,
    pub data_directory: String,
}

impl Configuration {
    pub fn from_environment() -> Self {
        Self {
            bind_address: std::env::var("QUIZAPP_BIND")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/quizapp.db?mode=rwc".to_string()),
            data_directory: std::env::var("QUIZAPP_DATA_DIR")
                .unwrap_or_else(|_| "data".to_string()),
        }
    }

    pub fn images_directory(&self) -> PathBuf {
        Path::new(&self.data_directory).join("images")
    }
}

// When bound to every interface, the bind address itself ("0.0.0.0:3000") is not a URL
// anyone can open. Finding the address a phone should use means asking the routing
// table which local interface would be chosen to reach the wider network. Connecting a
// UDP socket does that without sending a single packet, and needs no dependency and no
// working internet - only a configured route.
pub fn reachable_url(bind_address: &str) -> String {
    let Ok(bound) = bind_address.parse::<SocketAddr>() else {
        return format!("http://{bind_address}");
    };

    if !bound.ip().is_unspecified() {
        return format!("http://{bound}");
    }

    match local_network_address() {
        Some(address) => format!("http://{}:{}", address, bound.port()),
        None => format!("http://localhost:{}", bound.port()),
    }
}

fn local_network_address() -> Option<std::net::IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.168.1.1:80").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback() && !address.is_unspecified()).then_some(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_address_is_echoed_as_a_url() {
        assert_eq!(reachable_url("127.0.0.1:3000"), "http://127.0.0.1:3000");
        assert_eq!(reachable_url("192.168.2.161:3000"), "http://192.168.2.161:3000");
    }

    #[test]
    fn an_unparseable_address_still_produces_something_printable() {
        assert_eq!(reachable_url("nonsense"), "http://nonsense");
    }

    #[test]
    fn binding_every_interface_never_reports_the_unroutable_wildcard() {
        let url = reachable_url("0.0.0.0:3000");
        assert!(!url.contains("0.0.0.0"), "wildcard is not an openable address: {url}");
        assert!(url.ends_with(":3000"), "port must be preserved: {url}");
    }
}
