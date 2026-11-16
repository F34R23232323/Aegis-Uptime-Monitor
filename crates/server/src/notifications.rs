//! Validated data-only Discord templates. No scripting or arbitrary interpolation.
use crate::{
    auth::{User, write},
    *,
};
use axum::extract::{Extension, Path, State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, net::TcpStream};
use tokio_rustls::{TlsConnector, rustls::{ClientConfig, RootCertStore, pki_types::ServerName}};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbedTemplate {
    pub content: String,
    pub mention_users: bool,
    pub mention_roles: bool,
    pub mention_everyone: bool,
    pub title: String,
    pub description: String,
    pub footer: String,
    pub username: String,
    pub avatar_url: String,
    pub thumbnail_url: String,
    pub down_color: String,
    pub recovered_color: String,
    pub degraded_color: String,
    pub show_timestamp: bool,
    pub fields: Vec<EmbedField>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}
impl Default for EmbedTemplate {
    fn default() -> Self {
        Self {
            content: String::new(),
            mention_users: false,
            mention_roles: false,
            mention_everyone: false,
            title: "{{status}} · {{monitor}}".into(),
            description: "{{reason}}".into(),
            footer: "{{brand}}".into(),
            username: String::new(),
            avatar_url: String::new(),
            thumbnail_url: String::new(),
            down_color: "#ed4245".into(),
            recovered_color: "#57f287".into(),
            degraded_color: "#fee75c".into(),
            show_timestamp: true,
            fields: vec![
                EmbedField {
                    name: "Duration".into(),
                    value: "{{duration}}".into(),
                    inline: true,
                },
                EmbedField {
                    name: "Dashboard".into(),
                    value: "[Open monitor]({{dashboard_url}})".into(),
                    inline: true,
                },
            ],
        }
    }
}
fn valid_url(s: &str) -> bool {
    s.is_empty()
        || reqwest::Url::parse(s).is_ok_and(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
}
fn email_address(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else { return false; };
    !local.is_empty() && !domain.is_empty() && s.len() <= 254
        && local.bytes().all(|c| c.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&c))
        && domain.split('.').all(|part| !part.is_empty() && part.len() <= 63
            && !part.starts_with('-') && !part.ends_with('-')
            && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
        && domain.contains('.')
}
fn color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|v| v.is_ascii_hexdigit())
}
const TOKENS: [&str; 8] = [
    "monitor",
    "status",
    "reason",
    "duration",
    "timestamp",
    "dashboard_url",
    "brand",
    "incident_id",
];
fn validate_tokens(text: &str) -> bool {
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let v = &rest[start + 2..];
        let Some(end) = v.find("}}") else {
            return false;
        };
        if !TOKENS.contains(&v[..end].trim()) {
            return false;
        }
        rest = &v[end + 2..];
    }
    true
}
impl EmbedTemplate {
    pub fn validate(&self) -> Result<()> {
        if self.content.chars().count() > 2000
            || self.title.trim().is_empty()
            || self.title.len() > 256
            || self.description.len() > 4096
            || self.footer.len() > 2048
            || self.username.chars().count() > 80
            || self.avatar_url.len() > 2048
            || self.thumbnail_url.len() > 2048
            || !valid_url(&self.avatar_url)
            || !valid_url(&self.thumbnail_url)
            || ![
                &self.down_color,
                &self.recovered_color,
                &self.degraded_color,
            ]
            .iter()
            .all(|v| color(v))
            || self.fields.len() > 15
        {
            return Err(bad(
                "Invalid Discord text length, color, HTTPS image URL or field count",
            ));
        }
        let mut total = self.title.len() + self.description.len() + self.footer.len();
        for s in [&self.content, &self.title, &self.description, &self.footer] {
            if !validate_tokens(s) {
                return Err(bad(
                    "Template contains an unsupported or incomplete variable",
                ));
            }
        }
        for f in &self.fields {
            if f.name.trim().is_empty()
                || f.name.len() > 256
                || f.value.trim().is_empty()
                || f.value.len() > 1024
                || !validate_tokens(&f.name)
                || !validate_tokens(&f.value)
            {
                return Err(bad(
                    "Embed fields need valid names, values and supported variables",
                ));
            }
            total += f.name.len() + f.value.len();
        }
        if total > 5000 {
            return Err(bad(
                "Keep embed text below 5000 characters before variable expansion",
            ));
        }
        Ok(())
    }
}
fn trim_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
fn interpolate(s: &str, p: &Value, brand: &str) -> String {
    interpolate_with_mentions(s, p, brand, false)
}
fn interpolate_with_mentions(s: &str, p: &Value, brand: &str, escape_mentions: bool) -> String {
    let mut result = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let Some(end) = tail.find("}}") else {
            result.push_str(&rest[start..]);
            return result;
        };
        let key = tail[..end].trim();
        let value = match key {
            "monitor" => p["name"].as_str().unwrap_or("Monitor"),
            "status" => p["state"].as_str().unwrap_or("TEST"),
            "reason" => p["reason"].as_str().unwrap_or("Notification test"),
            "duration" => p["duration"].as_str().unwrap_or("Just detected"),
            "timestamp" => p["timestamp"].as_str().unwrap_or(""),
            "dashboard_url" => p["url"].as_str().unwrap_or(""),
            "brand" => brand,
            "incident_id" => p["incident_id"].as_str().unwrap_or(""),
            _ => "",
        };
        if escape_mentions {
            result.push_str(&value.replace('@', "@\u{200b}"));
        } else {
            result.push_str(value);
        }
        rest = &tail[end + 2..];
    }
    result.push_str(rest);
    result
}
pub fn render(template: &EmbedTemplate, p: &Value, brand: &str) -> Result<Value> {
    template.validate()?;
    let state = p["state"].as_str().unwrap_or("TEST");
    let c = match state {
        "DOWN" => &template.down_color,
        "DEGRADED" => &template.degraded_color,
        _ => &template.recovered_color,
    };
    let mut embed = json!({"title":trim_chars(&interpolate(&template.title,p,brand),256),"description":trim_chars(&interpolate(&template.description,p,brand),4096),"color":u32::from_str_radix(&c[1..],16).map_err(|_|bad("Invalid embed color"))?,"footer":{"text":trim_chars(&interpolate(&template.footer,p,brand),2048)}});
    if let Some(url) = p["url"].as_str().filter(|u| {
        reqwest::Url::parse(u)
            .is_ok_and(|u| ["http", "https"].contains(&u.scheme()) && u.host_str().is_some())
    }) {
        embed["url"] = json!(url);
    }
    if template.show_timestamp {
        embed["timestamp"] = p
            .get("timestamp")
            .cloned()
            .unwrap_or_else(|| json!(chrono::Utc::now().to_rfc3339()));
    }
    if !template.thumbnail_url.is_empty() {
        embed["thumbnail"] = json!({"url":template.thumbnail_url});
    }
    let fields:Vec<_>=template.fields.iter().map(|f|json!({"name":trim_chars(&interpolate(&f.name,p,brand),256),"value":trim_chars(&interpolate(&f.value,p,brand),1024),"inline":f.inline})).collect();
    if !fields.is_empty() {
        embed["fields"] = json!(fields);
    }
    let count = embed["title"].as_str().unwrap_or("").chars().count()
        + embed["description"].as_str().unwrap_or("").chars().count()
        + embed["footer"]["text"]
            .as_str()
            .unwrap_or("")
            .chars()
            .count()
        + fields
            .iter()
            .map(|f| {
                f["name"].as_str().unwrap_or("").chars().count()
                    + f["value"].as_str().unwrap_or("").chars().count()
            })
            .sum::<usize>();
    if count > 6000 {
        return Err(bad(
            "Expanded Discord embed exceeds 6000 characters; shorten the template",
        ));
    }
    let mut allowed = vec![];
    if template.mention_users {
        allowed.push("users");
    }
    if template.mention_roles {
        allowed.push("roles");
    }
    if template.mention_everyone {
        allowed.push("everyone");
    }
    let mut body = json!({"allowed_mentions":{"parse":allowed,"replied_user":false},"embeds":[embed]});
    if !template.content.is_empty() {
        body["content"] = json!(trim_chars(&interpolate_with_mentions(&template.content, p, brand, true), 2000));
    }
    if !template.username.is_empty() {
        body["username"] = json!(template.username);
    }
    if !template.avatar_url.is_empty() {
        body["avatar_url"] = json!(template.avatar_url);
    }
    Ok(body)
}
#[derive(Deserialize)]
pub struct ChannelInput {
    pub name: String,
    pub provider: String,
    pub secret: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub template: EmbedTemplate,
    #[serde(default = "default_events")]
    pub events: Vec<String>,
    #[serde(default)]
    pub monitor_ids: Vec<Uuid>,
    #[serde(default)]
    pub cooldown_seconds: i32,
    #[serde(default)]
    pub quiet_start_minute: Option<i32>,
    #[serde(default)]
    pub quiet_end_minute: Option<i32>,
    #[serde(default)]
    pub delay_seconds: i32,
    #[serde(default)]
    pub reminder_seconds: i32,
}
fn yes() -> bool {
    true
}
fn default_events() -> Vec<String> {
    vec!["DOWN".into(), "RECOVERED".into(), "DEGRADED".into()]
}
async fn validate_channel(a: &App, c: &ChannelInput) -> Result<()> {
    if c.name.trim().is_empty()
        || c.name.len() > 120
        || !["discord", "webhook", "email"].contains(&c.provider.as_str())
        || c.events.is_empty()
        || c.events.len() > 3
        || c.events
            .iter()
            .any(|e| !["DOWN", "RECOVERED", "DEGRADED"].contains(&e.as_str()))
        || c.monitor_ids.len() > 200
        || !(0..=86400).contains(&c.cooldown_seconds)
        || !(0..=86400).contains(&c.delay_seconds)
        || !(c.reminder_seconds == 0 || (300..=86400).contains(&c.reminder_seconds))
        || match (c.quiet_start_minute, c.quiet_end_minute) {
            (None, None) => false,
            (Some(start), Some(end)) => !(0..1440).contains(&start) || !(0..1440).contains(&end) || start == end,
            _ => true,
        }
    {
        return Err(bad(
            "Invalid destination, event selection, route or cooldown",
        ));
    }
    c.template.validate()?;
    if let Some(secret) = c.secret.as_ref().filter(|s| !s.is_empty()) {
        if c.provider == "email" {
            if !email_address(secret) {
                return Err(bad("Enter one valid email address"));
            }
        } else {
        if secret.len() > 2048 {
            return Err(bad("Webhook URL too long"));
        }
        let url = reqwest::Url::parse(secret).map_err(|_| bad("Invalid webhook URL"))?;
        if !["http", "https"].contains(&url.scheme())
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(bad(
                "Webhook must be an HTTP(S) URL without user information",
            ));
        }
        if c.provider == "discord"
            && (url.scheme() != "https"
                || url.host_str() != Some("discord.com")
                || url.port().is_some_and(|p| p != 443)
                || !url.path().starts_with("/api/webhooks/"))
        {
            return Err(bad("Use an HTTPS discord.com/api/webhooks URL"));
        }
        // Discord is a fixed HTTPS allowlisted host. Resolve and pin at delivery,
        // so editing presentation does not depend on external DNS availability.
        // Generic destinations still receive the explicit networking-policy check here.
        if c.provider == "webhook" {
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                aegis_checks::resolve(
                    url.host_str().ok_or_else(|| bad("Missing provider host"))?,
                    url.port_or_known_default()
                        .ok_or_else(|| bad("Missing port"))?,
                    a.private,
                ),
            )
            .await
            .map_err(|_| bad("Webhook validation timed out"))?
            .map_err(|_| bad("Webhook target is blocked or could not resolve"))?;
        }
        }
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM monitors WHERE id=ANY($1)")
        .bind(&c.monitor_ids)
        .fetch_one(&a.db)
        .await?;
    if count as usize != c.monitor_ids.len() {
        return Err(bad("Route contains missing or duplicate monitors"));
    }
    Ok(())
}
pub async fn channels(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
) -> Result<Json<Value>> {
    write(&u)?;
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(c)-'secret' ORDER BY c.created_at),'[]') FROM notification_channels c").fetch_one(&a.db).await?))
}
pub async fn create_channel(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(c): Json<ChannelInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    validate_channel(&a, &c).await?;
    let secret = c
        .secret
        .as_ref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("A destination is required"))?;
    let id = Uuid::new_v4();
    let mut tx = a.db.begin().await?;
    sqlx::query("INSERT INTO notification_channels(id,name,provider,secret,enabled,template,events,monitor_ids,cooldown_seconds,quiet_start_minute,quiet_end_minute,delay_seconds,reminder_seconds) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)").bind(id).bind(c.name.trim()).bind(c.provider).bind(encrypt(&a,secret)?).bind(c.enabled).bind(json!(c.template)).bind(c.events).bind(c.monitor_ids).bind(c.cooldown_seconds).bind(c.quiet_start_minute).bind(c.quiet_end_minute).bind(c.delay_seconds).bind(c.reminder_seconds).execute(&mut *tx).await?;
    crate::status::audit(&mut tx, &u, "channel.created", id, json!({})).await?;
    tx.commit().await?;
    Ok(Json(json!({"id":id})))
}
pub async fn update_channel(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(c): Json<ChannelInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    validate_channel(&a, &c).await?;
    let secret = c
        .secret
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| encrypt(&a, s))
        .transpose()?;
    let mut tx = a.db.begin().await?;
    let count=sqlx::query("UPDATE notification_channels SET name=$2,secret=coalesce($3,secret),enabled=$4,template=$5,events=$6,monitor_ids=$7,cooldown_seconds=$8,quiet_start_minute=$10,quiet_end_minute=$11,delay_seconds=$12,reminder_seconds=$13,updated_at=now() WHERE id=$1 AND provider=$9").bind(id).bind(c.name.trim()).bind(secret).bind(c.enabled).bind(json!(c.template)).bind(c.events).bind(c.monitor_ids).bind(c.cooldown_seconds).bind(c.provider).bind(c.quiet_start_minute).bind(c.quiet_end_minute).bind(c.delay_seconds).bind(c.reminder_seconds).execute(&mut *tx).await?.rows_affected();
    if count == 0 {
        return Err(bad(
            "Channel missing or provider changed; create a new channel to change provider",
        ));
    }
    crate::status::audit(&mut tx, &u, "channel.updated", id, json!({})).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct PreviewInput {
    #[serde(default)]
    pub template: EmbedTemplate,
    #[serde(default = "down")]
    pub event: String,
}
fn down() -> String {
    "DOWN".into()
}
pub async fn preview(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(p): Json<PreviewInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    if !["DOWN", "RECOVERED", "DEGRADED", "TEST"].contains(&p.event.as_str()) {
        return Err(bad("Invalid preview event"));
    }
    Ok(Json(render(
        &p.template,
        &json!({"name":"Example API","state":p.event,"reason":"Illustrative notification preview; no message is sent.","duration":"4m 12s","url":a.origin,"timestamp":chrono::Utc::now().to_rfc3339(),"incident_id":"example"}),
        &a.brand,
    )?))
}
pub async fn test_channel(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    let c = sqlx::query("SELECT template,enabled FROM notification_channels WHERE id=$1")
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Channel not found".into()))?;
    if !c.get::<bool, _>("enabled") {
        return Err(bad("Enable the channel before sending a test"));
    }
    let template: EmbedTemplate =
        serde_json::from_value(c.get("template")).map_err(|_| bad("Stored template is invalid"))?;
    let p = json!({"name":a.brand,"state":"TEST","reason":"Notification test","url":a.origin,"timestamp":chrono::Utc::now().to_rfc3339(),"duration":"Test message","template":template});
    sqlx::query(
        "INSERT INTO notification_deliveries(id,channel_id,event,payload) VALUES($1,$2,'TEST',$3)",
    )
    .bind(Uuid::new_v4())
    .bind(id)
    .bind(p)
    .execute(&a.db)
    .await?;
    Ok(Json(
        json!({"ok":true,"message":"Test queued. Delivery history records the provider result."}),
    ))
}
pub async fn retry_delivery(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    let changed=sqlx::query("UPDATE notification_deliveries d SET state='pending',attempts=0,due_at=now(),last_error=NULL WHERE d.id=$1 AND d.state='failed' AND EXISTS(SELECT 1 FROM notification_channels c WHERE c.id=d.channel_id AND c.enabled)").bind(id).execute(&a.db).await?.rows_affected();
    if changed == 0 {
        return Err(bad(
            "Only failed deliveries to enabled channels may be retried",
        ));
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn send_email(a: &App, address: &str, p: &Value) -> Result<()> {
    let host = std::env::var("SMTP_HOST").map_err(|_| bad("SMTP is not configured"))?;
    let from = std::env::var("SMTP_FROM").map_err(|_| bad("SMTP sender is not configured"))?;
    let port = std::env::var("SMTP_PORT").ok().and_then(|s| s.parse::<u16>().ok()).unwrap_or(587);
    if host.trim().is_empty() || port == 0 || !email_address(&from) || !email_address(address) {
        return Err(bad("SMTP host or port is invalid"));
    }
    let stream = TcpStream::connect((host.as_str(), port)).await.map_err(|_| bad("SMTP connection failed"))?;
    let mut connection = BufReader::new(stream);
    smtp_reply(&mut connection, 220).await?;
    connection.get_mut().write_all(b"EHLO aegis-monitor\r\n").await.map_err(|_| bad("SMTP write failed"))?;
    let capabilities = smtp_reply(&mut connection, 250).await?;
    if !capabilities.to_ascii_uppercase().contains("STARTTLS") {
        return Err(bad("SMTP server does not offer STARTTLS"));
    }
    connection.get_mut().write_all(b"STARTTLS\r\n").await.map_err(|_| bad("SMTP write failed"))?;
    smtp_reply(&mut connection, 220).await?;
    let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    let name = ServerName::try_from(host.clone()).map_err(|_| bad("SMTP host is invalid"))?;
    let tls = TlsConnector::from(std::sync::Arc::new(config)).connect(name, connection.into_inner()).await.map_err(|_| bad("SMTP TLS failed"))?;
    let mut connection = BufReader::new(tls);
    connection.get_mut().write_all(b"EHLO aegis-monitor\r\n").await.map_err(|_| bad("SMTP write failed"))?;
    smtp_reply(&mut connection, 250).await?;
    let user = std::env::var("SMTP_USERNAME").unwrap_or_default();
    let password = std::env::var("SMTP_PASSWORD").unwrap_or_default();
    if !user.is_empty() {
        let auth = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, format!("\0{user}\0{password}"));
        connection.get_mut().write_all(format!("AUTH PLAIN {auth}\r\n").as_bytes()).await.map_err(|_| bad("SMTP write failed"))?;
        smtp_reply(&mut connection, 235).await?;
    }
    for (command, expected) in [
        (format!("MAIL FROM:<{from}>\r\n"), 250),
        (format!("RCPT TO:<{address}>\r\n"), 250),
        ("DATA\r\n".to_string(), 354),
    ] {
        connection.get_mut().write_all(command.as_bytes()).await.map_err(|_| bad("SMTP write failed"))?;
        smtp_reply(&mut connection, expected).await?;
    }
    let subject = format!("{}: {} - {}", a.brand, p["state"].as_str().unwrap_or("UPDATE"), p["name"].as_str().unwrap_or("Monitor"))
        .replace('\r', " ").replace('\n', " ");
    let subject = if subject.is_ascii() { subject } else {
        format!("=?UTF-8?B?{}?=", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, subject))
    };
    let body = format!("{}\n\nStatus: {}\nReason: {}\nDuration: {}\n\n{}", p["name"].as_str().unwrap_or("Monitor"), p["state"].as_str().unwrap_or("UPDATE"), p["reason"].as_str().unwrap_or(""), p["duration"].as_str().unwrap_or(""), p["url"].as_str().unwrap_or(""));
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    let escaped = body.split('\n').map(|line| if line.starts_with('.') { format!(".{line}") } else { line.to_string() }).collect::<Vec<_>>().join("\r\n");
    let message = format!("From: <{from}>\r\nTo: <{address}>\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{escaped}\r\n.\r\n");
    connection.get_mut().write_all(message.as_bytes()).await.map_err(|_| bad("SMTP write failed"))?;
    smtp_reply(&mut connection, 250).await?;
    let _ = connection.get_mut().write_all(b"QUIT\r\n").await;
    Ok(())
}
async fn smtp_reply<T: tokio::io::AsyncRead + Unpin>(stream: &mut BufReader<T>, expected: u16) -> Result<String> {
    let mut result = String::new();
    for _ in 0..20 {
        let mut line = String::new();
        let bytes = stream.read_line(&mut line).await.map_err(|_| bad("SMTP read failed"))?;
        if bytes == 0 || bytes > 2048 || result.len() + bytes > 8192 || line.len() < 4 {
            return Err(bad("SMTP reply is invalid"));
        }
        let code = line.get(..3).and_then(|value| value.parse::<u16>().ok()).ok_or_else(|| bad("SMTP reply is invalid"))?;
        if code != expected { return Err(bad("SMTP server rejected request")); }
        let done = line.as_bytes()[3] == b' ';
        result.push_str(&line);
        if done { return Ok(result); }
        if line.as_bytes()[3] != b'-' { return Err(bad("SMTP reply is invalid")); }
    }
    Err(bad("SMTP reply is too long"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_mention_delivery() {
        let t = EmbedTemplate::default();
        let p = render(
            &t,
            &json!({"name":"@everyone {{status}}","state":"DOWN","url":"https://example.com"}),
            "Brand",
        )
        .unwrap();
        assert_eq!(p["allowed_mentions"]["parse"], json!([]));
        assert_eq!(p["embeds"][0]["title"], "DOWN · @everyone {{status}}");
    }
    #[test]
    fn message_content_renders_outside_embed_without_mentions() {
        let t = EmbedTemplate {
            content: "@everyone {{status}}: {{monitor}}".into(),
            ..Default::default()
        };
        let p = render(&t, &json!({"name":"API","state":"DOWN"}), "Brand").unwrap();
        assert_eq!(p["content"], "@everyone DOWN: API");
        assert_eq!(p["allowed_mentions"]["parse"], json!([]));
        assert_eq!(p["embeds"][0]["title"], "DOWN · API");
        assert!(serde_json::from_value::<EmbedTemplate>(json!({"title":"Old channel"})).is_ok());
    }
    #[test]
    fn enabled_mentions_only_parse_authored_content() {
        let t = EmbedTemplate {
            content: "<@123456789012345678> <@&234567890123456789> @everyone {{monitor}}".into(),
            mention_users: true,
            mention_roles: true,
            mention_everyone: true,
            ..Default::default()
        };
        let p = render(
            &t,
            &json!({"name":"@here <@345678901234567890>","state":"DOWN"}),
            "Brand",
        )
        .unwrap();
        assert_eq!(p["allowed_mentions"]["parse"], json!(["users", "roles", "everyone"]));
        assert_eq!(p["content"], "<@123456789012345678> <@&234567890123456789> @everyone @\u{200b}here <@\u{200b}345678901234567890>");
    }
    #[test]
    fn reject_executable_or_unknown_fields() {
        let mut t = EmbedTemplate {
            title: "{{secret}}".into(),
            ..Default::default()
        };
        assert!(t.validate().is_err());
        t.title = "{{status}}".into();
        t.avatar_url = "javascript:alert(1)".into();
        assert!(t.validate().is_err());
    }
}
