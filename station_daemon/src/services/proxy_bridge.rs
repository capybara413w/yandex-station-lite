use std::sync::Arc;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_socks::tcp::Socks5Stream;
use tracing::{info, warn};
use url::Url;

use crate::config::AppConfig;

pub struct ProxyBridge {
    config: Arc<RwLock<AppConfig>>,
}

impl ProxyBridge {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self { config }
    }

    /// Spawns a background HTTP CONNECT tunnel on 127.0.0.1:8889 that forwards to the configured SOCKS5 proxy
    pub fn start(self: Arc<Self>, bind_port: u16) {
        tokio::spawn(async move {
            let addr_str = format!("127.0.0.1:{}", bind_port);
            let sock_addr: std::net::SocketAddr = match addr_str.parse() {
                Ok(a) => a,
                Err(e) => {
                    warn!("Invalid proxy bridge bind address {}: {}", addr_str, e);
                    return;
                }
            };

            let mut listener = None;
            for _attempt in 1..=20 {
                if let Ok(socket) = tokio::net::TcpSocket::new_v4() {
                    let _ = socket.set_reuseaddr(true);
                    let _ = socket.set_reuseport(true);
                    if socket.bind(sock_addr).is_ok() {
                        if let Ok(l) = socket.listen(128) {
                            info!("Local HTTP-to-SOCKS5 audio proxy bridge listening on {}", addr_str);
                            listener = Some(l);
                            break;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }

            let listener = match listener {
                Some(l) => l,
                None => {
                    warn!("Failed to bind local HTTP proxy bridge on {} after multiple attempts", addr_str);
                    return;
                }
            };

            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let self_clone = self.clone();
                        tokio::spawn(async move {
                            if let Err(e) = self_clone.handle_client(stream).await {
                                warn!("Proxy tunnel error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Proxy bridge listener error: {}", e);
                    }
                }
            }
        });
    }

    async fn handle_client(&self, mut client: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Read initial HTTP request header (e.g. CONNECT host:port HTTP/1.1)
        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }

        let header_str = String::from_utf8_lossy(&buf[..n]);
        let mut lines = header_str.lines();
        let first_line = lines.next().unwrap_or("");

        let mut parts = first_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let target_raw = parts.next().unwrap_or("");

        // Parse proxy credentials from config
        let (socks5_url_str, proxy_enabled) = {
            let cfg = self.config.read().await;
            (cfg.proxy.socks5_url.clone(), cfg.proxy.enabled)
        };

        if !proxy_enabled || socks5_url_str.is_empty() {
            client.write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\nProxy Disabled").await?;
            return Ok(());
        }

        let proxy_url = Url::parse(&socks5_url_str)?;
        let proxy_host = proxy_url.host_str().unwrap_or("127.0.0.1");
        let proxy_port = proxy_url.port().unwrap_or(1080);
        let username = proxy_url.username();
        let password = proxy_url.password().unwrap_or("");

        let proxy_target_addr = format!("{}:{}", proxy_host, proxy_port);

        // Determine destination target host & port
        let (dest_host, dest_port) = if method.eq_ignore_ascii_case("CONNECT") {
            let mut hp = target_raw.split(':');
            let host = hp.next().unwrap_or("");
            let port = hp.next().and_then(|p| p.parse::<u16>().ok()).unwrap_or(443);
            (host.to_string(), port)
        } else if target_raw.starts_with("http://") || target_raw.starts_with("https://") {
            let u = Url::parse(target_raw)?;
            let host = u.host_str().unwrap_or("").to_string();
            let port = u.port_or_known_default().unwrap_or(80);
            (host, port)
        } else {
            client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nInvalid proxy request").await?;
            return Ok(());
        };

        if dest_host.is_empty() {
            client.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nMissing host").await?;
            return Ok(());
        }

        // Connect to destination through remote SOCKS5 proxy
        let mut server = if !username.is_empty() {
            Socks5Stream::connect_with_password(
                proxy_target_addr.as_str(),
                (dest_host.as_str(), dest_port),
                username,
                password,
            )
            .await?
        } else {
            Socks5Stream::connect(proxy_target_addr.as_str(), (dest_host.as_str(), dest_port)).await?
        };

        if method.eq_ignore_ascii_case("CONNECT") {
            client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
        } else {
            server.write_all(&buf[..n]).await?;
        }

        let _ = copy_bidirectional(&mut client, &mut server).await;

        Ok(())
    }
}
