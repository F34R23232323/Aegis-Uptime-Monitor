use aegis_core::MonitorInput;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::{
    net::{IpAddr, SocketAddr},
    time::Instant,
};
use tokio::{
    net::{TcpStream, lookup_host},
    time::{Duration, timeout},
};
use url::Url;
#[derive(Debug, Clone)]
pub struct Observation {
    pub ok: bool,
    pub latency_ms: f64,
    pub error: Option<String>,
    pub observed: Value,
}
impl Observation {
    pub fn from_error(message: &str) -> Self {
        Self {
            ok: false,
            latency_ms: 0.0,
            error: Some(message.into()),
            observed: json!({}),
        }
    }
}
fn public_ip(ip: IpAddr) -> bool {
    static DENY: std::sync::OnceLock<Vec<ipnet::IpNet>> = std::sync::OnceLock::new();
    let deny = DENY.get_or_init(|| {
        [
            "0.0.0.0/8",
            "10.0.0.0/8",
            "100.64.0.0/10",
            "127.0.0.0/8",
            "169.254.0.0/16",
            "172.16.0.0/12",
            "192.0.0.0/24",
            "192.0.2.0/24",
            "192.88.99.0/24",
            "192.168.0.0/16",
            "198.18.0.0/15",
            "198.51.100.0/24",
            "203.0.113.0/24",
            "224.0.0.0/4",
            "240.0.0.0/4",
            "2001::/32",
            "2001:db8::/32",
            "2001:10::/28",
            "2002::/16",
        ]
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect()
    });
    let ip = match ip {
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                IpAddr::V4(v4)
            } else {
                if v.segments()[0] & 0xe000 != 0x2000 {
                    return false;
                }
                IpAddr::V6(v)
            }
        }
        v => v,
    };
    !deny.iter().any(|network| network.contains(&ip))
}
pub async fn resolve(host: &str, port: u16, allow_private: bool) -> Result<Vec<SocketAddr>> {
    let addrs: Vec<_> = lookup_host((host.trim_matches(['[', ']']), port))
        .await?
        .take(16)
        .collect();
    if addrs.is_empty() {
        bail!("Target has no addresses");
    }
    if !allow_private && addrs.iter().any(|a| !public_ip(a.ip())) {
        bail!("Private, local and reserved targets require ALLOW_PRIVATE_TARGETS=true");
    }
    Ok(addrs)
}
pub async fn validate_target(m: &MonitorInput, allow_private: bool) -> Result<()> {
    m.validate().map_err(|e| anyhow!(e))?;
    match m.kind.as_str() {
        "http" | "json" => {
            let u = Url::parse(&m.target)?;
            if !["http", "https"].contains(&u.scheme())
                || !u.username().is_empty()
                || u.password().is_some()
            {
                bail!("Only HTTP(S) URLs without embedded credentials are allowed");
            }
            resolve(
                u.host_str().context("Missing host")?,
                u.port_or_known_default().context("Missing port")?,
                allow_private,
            )
            .await?;
        }
        "tcp" | "tls" | "redis" | "smtp" | "postgres" => {
            let u = Url::parse(&format!("tcp://{}", m.target))?;
            resolve(
                u.host_str().context("Missing host")?,
                u.port().context("Use host:port")?,
                allow_private,
            )
            .await?;
        }
        "dns" if (m.target.is_empty() || m.target.contains(['/', ':', ' '])) => {
            bail!("Use a DNS hostname");
        }
        _ => {}
    }
    Ok(())
}
pub async fn execute(m: &MonitorInput, allow_private: bool) -> Observation {
    let start = Instant::now();
    let result = timeout(
        Duration::from_secs(m.timeout_seconds as u64),
        run(m, allow_private),
    )
    .await;
    let (ok, error, observed) = match result {
        Ok(Ok(v)) => (true, None, v),
        Ok(Err(e)) => (false, Some(safe_error(&e)), json!({})),
        Err(_) => (false, Some("Check timed out".into()), json!({})),
    };
    Observation {
        ok,
        error,
        observed,
        latency_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}
fn safe_error(e: &anyhow::Error) -> String {
    let message = e.to_string();
    if message.starts_with("HTTP status ")
        || message.starts_with("Certificate expires in ")
        || [
            "Keyword missing",
            "Body exceeds 256 KiB keyword limit",
            "Body exceeds 256 KiB matching limit",
            "Body exceeds 2 MiB matching limit",
            "Body exceeds 1 MiB matching limit",
            "Response is not valid JSON",
            "JSON assertion did not match",
            "Status component missing",
            "Status component is not operational",
            "Google Cloud product has an active public incident",
            "Expected DNS value absent",
            "Redis PING was not acknowledged",
            "SMTP greeting was not accepted",
            "PostgreSQL protocol handshake failed",
            "Protocol greeting exceeded the bounded line limit",
            "Expected address absent",
            "Target has no addresses",
            "Expired certificate",
            "Private, local and reserved targets require ALLOW_PRIVATE_TARGETS=true",
        ]
        .contains(&message.as_str())
    {
        return message;
    }
    if let Some(http) = e.downcast_ref::<reqwest::Error>() {
        return if http.is_timeout() {
            "HTTP request timed out"
        } else if http.is_connect() {
            "HTTP connection failed (TCP or TLS handshake)"
        } else if http.is_body() {
            "HTTP response body could not be read"
        } else {
            "HTTP request failed"
        }
        .into();
    }
    if let Some(io) = e.downcast_ref::<std::io::Error>() {
        return match io.kind() {
            std::io::ErrorKind::ConnectionRefused => "Connection refused",
            std::io::ErrorKind::TimedOut => "Connection timed out",
            std::io::ErrorKind::NotFound => "Address or service not found",
            _ => "Network connection, name resolution or TLS handshake failed",
        }
        .into();
    }
    "DNS resolution, TLS certificate validation or connection failed".into()
}
fn check_status_component(json: &Value, id: &str) -> Result<()> {
    let component = json["components"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["id"] == id))
        .ok_or_else(|| anyhow!("Status component missing"))?;
    if component["status"] != "operational" {
        bail!("Status component is not operational");
    }
    Ok(())
}
fn check_google_product(json: &Value, id: &str) -> Result<()> {
    let incidents = json.as_array().ok_or_else(|| anyhow!("Response is not valid JSON"))?;
    if incidents.iter().any(|incident| {
        incident["end"].is_null()
            && matches!(incident["status_impact"].as_str(), Some("SERVICE_OUTAGE" | "SERVICE_DISRUPTION"))
            && incident["affected_products"]
                .as_array()
                .is_some_and(|products| products.iter().any(|product| product["id"] == id))
    }) {
        bail!("Google Cloud product has an active public incident");
    }
    Ok(())
}
async fn run(m: &MonitorInput, allow_private: bool) -> Result<Value> {
    match m.kind.as_str() {
        "http" | "json" => {
            let mut u = Url::parse(&m.target)?;
            let initial_origin = u.origin();
            let method = if m.config["method"] == "HEAD" {
                reqwest::Method::HEAD
            } else {
                reqwest::Method::GET
            };
            let mut redirects = 0;
            let mut response = loop {
                if !["http", "https"].contains(&u.scheme())
                    || !u.username().is_empty()
                    || u.password().is_some()
                    || u.as_str().len() > 2048
                {
                    bail!("Redirect target must be a bounded HTTP(S) URL without credentials");
                }
                let host = u.host_str().context("Missing host")?;
                let addrs = resolve(
                    host,
                    u.port_or_known_default().context("Missing port")?,
                    allow_private,
                )
                .await?;
                let client = reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .no_proxy()
                    .resolve_to_addrs(host, &addrs)
                    .timeout(Duration::from_secs(m.timeout_seconds as u64))
                    .build()?;
                let mut request = client.request(method.clone(), u.clone());
                if u.origin() == initial_origin
                    && let Some(c) = m.config.get("_credentials")
                {
                    if let Some(token) = c["bearer"].as_str().filter(|s| !s.is_empty()) {
                        request = request.bearer_auth(token);
                    } else if let Some(username) =
                        c["basic_username"].as_str().filter(|s| !s.is_empty())
                    {
                        request = request.basic_auth(username, c["basic_password"].as_str());
                    }
                    for h in c["headers"].as_array().into_iter().flatten() {
                        request = request.header(
                            h["name"].as_str().context("Invalid stored header")?,
                            h["value"].as_str().context("Invalid stored header")?,
                        );
                    }
                }
                let response = request.send().await?;
                if m.config["follow_redirects"] == true && response.status().is_redirection() {
                    if redirects >= 3 {
                        bail!("HTTP redirect limit exceeded");
                    }
                    let location = response
                        .headers()
                        .get(reqwest::header::LOCATION)
                        .context("HTTP redirect missing Location")?
                        .to_str()?;
                    u = u.join(location)?;
                    redirects += 1;
                    continue;
                }
                break response;
            };
            let status = response.status().as_u16();
            let expected = m.config["status"].as_u64().unwrap_or(200) as u16;
            if status != expected {
                bail!("HTTP status {status}; expected {expected}");
            }
            let keyword = m.config["keyword"].as_str().filter(|s| !s.is_empty());
            if keyword.is_some() || m.kind == "json" {
                let mut body = Vec::new();
                while let Some(chunk) = response.chunk().await? {
                    let body_limit = if m.config.get("json_google_product_id").is_some() {
                        2 * 1024 * 1024
                    } else if m.config.get("json_component_id").is_some() {
                        1024 * 1024
                    } else {
                        262144
                    };
                    if body.len() + chunk.len() > body_limit {
                        if body_limit == 2 * 1024 * 1024 {
                            bail!("Body exceeds 2 MiB matching limit");
                        } else if body_limit == 1024 * 1024 {
                            bail!("Body exceeds 1 MiB matching limit");
                        }
                        bail!("Body exceeds 256 KiB matching limit");
                    }
                    body.extend_from_slice(&chunk);
                }
                if let Some(keyword) = keyword
                    && !String::from_utf8_lossy(&body).contains(keyword)
                {
                    bail!("Keyword missing");
                }
                if m.kind == "json" {
                    let json: Value = serde_json::from_slice(&body)
                        .map_err(|_| anyhow!("Response is not valid JSON"))?;
                    if let Some(id) = m.config["json_component_id"].as_str() {
                        check_status_component(&json, id)?;
                    } else if let Some(id) = m.config["json_google_product_id"].as_str() {
                        check_google_product(&json, id)?;
                    } else {
                        let pointer = m.config["json_pointer"].as_str().unwrap_or("");
                        if json.pointer(pointer) != m.config.get("json_expected") {
                            bail!("JSON assertion did not match");
                        }
                    }
                }
            }
            Ok(json!({"status":status}))
        }
        "tcp" => {
            let u = Url::parse(&format!("tcp://{}", m.target))?;
            let addrs = resolve(
                u.host_str().context("Missing host")?,
                u.port().context("Missing port")?,
                allow_private,
            )
            .await?;
            TcpStream::connect(addrs.as_slice()).await?;
            Ok(json!({"connected":true}))
        }
        "dns" => {
            use hickory_resolver::{Resolver, proto::rr::RecordType};
            let resolver = Resolver::builder_tokio()?.build()?;
            let record = m.config["record_type"].as_str().unwrap_or("IP");
            let values: Vec<String> = if record == "IP" {
                resolver
                    .lookup_ip(m.target.as_str())
                    .await?
                    .iter()
                    .take(100)
                    .map(|v| v.to_string())
                    .collect()
            } else {
                let kind: RecordType = record
                    .parse()
                    .map_err(|_| anyhow!("Unsupported DNS record type"))?;
                resolver
                    .lookup(m.target.as_str(), kind)
                    .await?
                    .answers()
                    .iter()
                    .take(100)
                    .map(|v| v.data.to_string())
                    .collect()
            };
            if values.is_empty() {
                bail!("Target has no addresses");
            }
            if let Some(expected) = m.config["expected"].as_str().filter(|s| !s.is_empty())
                && !values.iter().any(|v| {
                    v.trim_end_matches('.')
                        .eq_ignore_ascii_case(expected.trim_end_matches('.'))
                })
            {
                bail!("Expected DNS value absent");
            }
            Ok(json!({"record_type":record,"records":values}))
        }
        "redis" | "smtp" | "postgres" => {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let u = Url::parse(&format!("tcp://{}", m.target))?;
            let addresses = resolve(
                u.host_str().context("Missing host")?,
                u.port().context("Missing port")?,
                allow_private,
            )
            .await?;
            let mut stream = TcpStream::connect(addresses.as_slice()).await?;
            if m.kind == "postgres" {
                stream.write_all(&[0, 0, 0, 8, 4, 210, 22, 47]).await?;
                let mut byte = [0u8; 1];
                stream.read_exact(&mut byte).await?;
                if !b"SN".contains(&byte[0]) {
                    bail!("PostgreSQL protocol handshake failed");
                }
            } else {
                use tokio::io::AsyncBufReadExt;
                if m.kind == "redis" {
                    stream.write_all(b"*1\r\n$4\r\nPING\r\n").await?;
                }
                let mut reader = tokio::io::BufReader::new(stream.take(513));
                let mut reply = Vec::new();
                reader.read_until(b'\n', &mut reply).await?;
                if reply.len() > 512 || !reply.ends_with(b"\n") {
                    bail!("Protocol greeting exceeded the bounded line limit");
                }
                if m.kind == "redis" && reply != b"+PONG\r\n" {
                    bail!("Redis PING was not acknowledged");
                }
                if m.kind == "smtp" && !reply.starts_with(b"220 ") && !reply.starts_with(b"220-") {
                    bail!("SMTP greeting was not accepted");
                }
            }
            Ok(json!({"protocol":m.kind,"handshake":true}))
        }
        "tls" => {
            use tokio_rustls::{
                TlsConnector,
                rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
            };
            let u = Url::parse(&format!("tls://{}", m.target))?;
            let host = u.host_str().context("Missing host")?.to_owned();
            let addrs = resolve(&host, u.port().unwrap_or(443), allow_private).await?;
            let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let stream = TcpStream::connect(addrs.as_slice()).await?;
            let tls = TlsConnector::from(std::sync::Arc::new(config))
                .connect(ServerName::try_from(host)?, stream)
                .await?;
            let cert = tls
                .get_ref()
                .1
                .peer_certificates()
                .and_then(|c| c.first())
                .context("No certificate")?;
            let (_, cert) = x509_parser::parse_x509_certificate(cert.as_ref())
                .map_err(|_| anyhow!("Invalid certificate"))?;
            let days = cert
                .validity()
                .time_to_expiration()
                .context("Expired certificate")?
                .whole_days();
            if days < m.config["warning_days"].as_i64().unwrap_or(14) {
                bail!("Certificate expires in {days} days; warning threshold reached");
            }
            Ok(
                json!({"days_remaining":days,"expires_at":cert.validity().not_after.to_rfc2822().map_err(|e|anyhow!(e))?}),
            )
        }
        _ => bail!("This monitor is evaluated by heartbeat or enrolled agent"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_component_is_matched_by_id_not_position() {
        let body = json!({"components":[
            {"id":"other","status":"major_outage"},
            {"id":"wanted","status":"operational"}
        ]});
        assert!(check_status_component(&body, "wanted").is_ok());
        assert!(check_status_component(&body, "other").is_err());
        assert!(check_status_component(&body, "removed").is_err());
    }
    #[test]
    fn google_product_uses_only_active_incidents() {
        let resolved = json!([{"end":"2026-01-01T00:00:00Z","affected_products":[{"id":"wanted"}]}]);
        assert!(check_google_product(&resolved, "wanted").is_ok());
        let active = json!([{"end":null,"status_impact":"SERVICE_DISRUPTION","affected_products":[{"id":"wanted"}]}]);
        assert!(check_google_product(&active, "wanted").is_err());
        assert!(check_google_product(&active, "other").is_ok());
    }
    #[test]
    fn address_policy() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()));
        }
        assert!(public_ip("1.1.1.1".parse().unwrap()));
    }
}
#[cfg(test)]
mod network_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn monitor(kind: &str, target: String) -> MonitorInput {
        MonitorInput {
            name: "test".into(),
            kind: kind.into(),
            target,
            interval_seconds: 20,
            timeout_seconds: 2,
            failure_threshold: 1,
            recovery_threshold: 1,
            group_name: String::new(),
            description: String::new(),
            tags: vec![],
            public: false,
            server_id: None,
            config: json!({}),
        }
    }
    async fn http(response: String) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await;
            let _ = stream.write_all(response.as_bytes()).await;
        });
        (format!("http://{address}/"), task)
    }
    #[tokio::test]
    async fn real_http_success_and_status_failure() {
        let (url, task) = http("HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nhealthy".into()).await;
        let mut m = monitor("http", url);
        m.config = json!({"keyword":"healthy"});
        assert!(execute(&m, true).await.ok);
        task.await.unwrap();
        let (url, task) =
            http("HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n".into()).await;
        assert!(!execute(&monitor("http", url), true).await.ok);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn redirects_are_not_followed() {
        let (url, task) = http(
            "HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/\r\nContent-Length: 0\r\n\r\n"
                .into(),
        )
        .await;
        assert!(!execute(&monitor("http", url), true).await.ok);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn redirects_are_bounded_and_opt_in() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            for step in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 1024];
                let _ = stream.read(&mut request).await.unwrap();
                let reply = if step == 0 {
                    "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
                };
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let mut m = monitor("http", format!("http://{address}/"));
        m.config = json!({"follow_redirects":true});
        assert!(execute(&m, true).await.ok);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn redirects_do_not_forward_saved_credentials_to_another_origin() {
        let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let destination_address = destination.local_addr().unwrap();
        let destination_task = tokio::spawn(async move {
            let (mut stream, _) = destination.accept().await.unwrap();
            let mut request = [0; 2048];
            let count = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
            assert!(!request.contains("authorization:"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });
        let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source_address = source.local_addr().unwrap();
        let source_task = tokio::spawn(async move {
            let (mut stream, _) = source.accept().await.unwrap();
            let mut request = [0; 2048];
            let count = stream.read(&mut request).await.unwrap();
            assert!(
                String::from_utf8_lossy(&request[..count])
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret")
            );
            stream.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://{destination_address}/\r\nContent-Length: 0\r\n\r\n").as_bytes()).await.unwrap();
        });
        let mut m = monitor("http", format!("http://{source_address}/"));
        m.config = json!({"follow_redirects":true,"_credentials":{"bearer":"secret"}});
        assert!(execute(&m, true).await.ok);
        source_task.await.unwrap();
        destination_task.await.unwrap();
    }
    #[tokio::test]
    async fn bounded_keyword_body() {
        let body = "x".repeat(262145);
        let (url, task) = http(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await;
        let mut m = monitor("http", url);
        m.config = json!({"keyword":"missing"});
        assert!(!execute(&m, true).await.ok);
        task.await.unwrap();
    }
    #[tokio::test]
    async fn real_tcp_and_private_policy() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = listener.local_addr().unwrap().to_string();
        let m = monitor("tcp", target);
        assert!(execute(&m, true).await.ok);
        assert!(!execute(&m, false).await.ok);
        drop(listener);
        assert!(!execute(&m, true).await.ok);
    }
    #[tokio::test]
    async fn dns_error_is_a_failure() {
        let m = monitor("dns", "nonexistent-aegis-test.invalid".into());
        assert!(!execute(&m, false).await.ok);
    }
}
#[cfg(test)]
mod protocol_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn monitor(kind: &str, target: String) -> MonitorInput {
        MonitorInput {
            name: "Protocol test".into(),
            kind: kind.into(),
            target,
            interval_seconds: 20,
            timeout_seconds: 2,
            failure_threshold: 1,
            recovery_threshold: 1,
            group_name: String::new(),
            description: String::new(),
            tags: vec![],
            public: false,
            server_id: None,
            config: json!({}),
        }
    }
    #[tokio::test]
    async fn json_assertions_use_bounded_authenticated_http() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = [0u8; 4096];
                let n = stream.read(&mut bytes).await.unwrap();
                assert!(
                    String::from_utf8_lossy(&bytes[..n])
                        .to_lowercase()
                        .contains("authorization: bearer test-secret")
                );
                let body = r#"{"health":{"ready":true}}"#;
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
        });
        let mut m = monitor("json", format!("http://{address}/"));
        m.config = json!({"json_pointer":"/health/ready","json_expected":true,"_credentials":{"bearer":"test-secret"}});
        assert!(execute(&m, true).await.ok);
        m.config["json_expected"] = json!(false);
        let result = execute(&m, true).await;
        assert!(!result.ok);
        assert_eq!(
            result.error.as_deref(),
            Some("JSON assertion did not match")
        );
        task.await.unwrap();
    }
    #[tokio::test]
    async fn real_redis_smtp_and_postgres_protocols() {
        for (kind, response) in [
            ("redis", b"+PONG\r\n".as_slice()),
            ("smtp", b"220 test.example ESMTP\r\n".as_slice()),
            ("postgres", b"S".as_slice()),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let reply = response.to_vec();
            let k = kind.to_owned();
            let task = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                if k != "smtp" {
                    let mut request = [0u8; 64];
                    let n = stream.read(&mut request).await.unwrap();
                    if k == "redis" {
                        assert_eq!(&request[..n], b"*1\r\n$4\r\nPING\r\n");
                    } else {
                        assert_eq!(&request[..n], &[0, 0, 0, 8, 4, 210, 22, 47]);
                    }
                }
                stream.write_all(&reply).await.unwrap();
            });
            assert!(execute(&monitor(kind, address.to_string()), true).await.ok);
            task.await.unwrap();
        }
    }
    #[tokio::test]
    async fn invalid_protocol_reply_is_not_reported_healthy() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(b"500 wrong protocol\r\n").await.unwrap();
        });
        assert!(
            !execute(&monitor("smtp", address.to_string()), true)
                .await
                .ok
        );
        task.await.unwrap();
    }
    #[test]
    fn reserved_ranges_are_blocked_without_rejecting_entire_public_prefixes() {
        for ip in [
            "100.64.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "203.0.113.5",
            "2001:db8::1",
            "::ffff:169.254.169.254",
        ] {
            assert!(!public_ip(ip.parse().unwrap()));
        }
        assert!(public_ip("100.1.1.1".parse().unwrap()));
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
#[cfg(test)]
mod fragmented_protocol_tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    #[tokio::test]
    async fn smtp_greeting_may_arrive_in_multiple_packets() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(b"2").await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
            stream.write_all(b"20 example.test SMTP\r\n").await.unwrap();
        });
        let m = MonitorInput {
            name: "SMTP".into(),
            kind: "smtp".into(),
            target: address.to_string(),
            interval_seconds: 20,
            timeout_seconds: 2,
            failure_threshold: 1,
            recovery_threshold: 1,
            group_name: String::new(),
            description: String::new(),
            tags: vec![],
            public: false,
            server_id: None,
            config: json!({}),
        };
        assert!(execute(&m, true).await.ok);
        task.await.unwrap();
    }
}
