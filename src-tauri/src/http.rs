use reqwest::Client;
use std::sync::OnceLock;

static CLIENT: OnceLock<Client> = OnceLock::new();

/// One shared client: reuses the connection pool (no DNS + TLS handshake per request).
pub fn client() -> &'static Client {
    CLIENT.get_or_init(Client::new)
}
