mod api;
mod auth;
mod credentials;
mod engine;
mod notifications;
mod status;
use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::sync::broadcast;
#[derive(Clone)]
pub struct App {
    pub db: sqlx::PgPool,
    pub events: broadcast::Sender<String>,
    pub origin: String,
    pub key: [u8; 32],
    pub private: bool,
    pub secure: bool,
    pub brand: String,
}
#[derive(Debug)]
pub struct Error(pub StatusCode, pub String);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(code=?e.as_database_error().and_then(|e|e.code()),"database operation failed");
        let code = e.as_database_error().and_then(|e| e.code());
        match code.as_deref() {
            Some("23505") => Self(
                StatusCode::CONFLICT,
                "A resource with that unique value already exists".into(),
            ),
            Some("23503") => Self(
                StatusCode::CONFLICT,
                "The resource is still referenced, or a referenced resource was removed".into(),
            ),
            _ => Self(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database operation failed".into(),
            ),
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn bad(s: &str) -> Error {
    Error(StatusCode::BAD_REQUEST, s.into())
}
pub fn app_accent() -> String {
    let color = std::env::var("APP_ACCENT").unwrap_or_else(|_| "#a18aff".into());
    if color.len() == 7
        && color.starts_with('#')
        && color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        color
    } else {
        "#a18aff".into()
    }
}
pub fn secret() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    hex::encode(b)
}
pub fn hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(s.as_bytes()))
}
pub fn encrypt(app: &App, s: &str) -> Result<String> {
    let cipher = Aes256Gcm::new((&app.key).into());
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt((&nonce).into(), s.as_bytes())
        .map_err(|_| bad("Encryption failed"))?;
    Ok(STANDARD.encode([nonce.to_vec(), ciphertext].concat()))
}
pub fn decrypt(app: &App, s: &str) -> Result<String> {
    let b = STANDARD
        .decode(s)
        .map_err(|_| bad("Invalid encrypted secret"))?;
    if b.len() < 28 {
        return Err(bad("Invalid encrypted secret"));
    }
    let cipher = Aes256Gcm::new((&app.key).into());
    let bytes = cipher
        .decrypt((&b[..12]).into(), &b[12..])
        .map_err(|_| bad("Unable to decrypt secret"))?;
    String::from_utf8(bytes).map_err(|_| bad("Invalid secret"))
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let key = hex::decode(std::env::var("ENCRYPTION_KEY").map_err(|_| {
        anyhow::anyhow!("ENCRYPTION_KEY is required: generate 32 random bytes as hex")
    })?)?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| anyhow::anyhow!("ENCRYPTION_KEY must be 64 hex characters"))?;
    anyhow::ensure!(
        key.iter().any(|b| *b != key[0]),
        "Use a randomly generated encryption key, not repeated bytes"
    );
    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .after_connect(|conn, _| {
            Box::pin(async move {
                for statement in [
                    "SET statement_timeout=15000",
                    "SET lock_timeout=5000",
                    "SET idle_in_transaction_session_timeout=15000",
                ] {
                    sqlx::query(statement).execute(&mut *conn).await?;
                }
                Ok(())
            })
        })
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    sqlx::migrate!("../../migrations").run(&db).await?;
    if std::env::args().nth(1).as_deref() == Some("reset-password") {
        auth::recover(&db).await?;
        return Ok(());
    }
    // A session-level PostgreSQL lock enforces one active engine, held for process lifetime.
    let mut leader = db.acquire().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(871221)")
        .fetch_one(&mut *leader)
        .await?;
    anyhow::ensure!(
        locked,
        "Another active Aegis server holds the scheduler lock"
    );
    let origin = std::env::var("APP_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into())
        .trim_end_matches('/')
        .to_owned();
    let parsed = reqwest::Url::parse(&origin)?;
    anyhow::ensure!(
        ["http", "https"].contains(&parsed.scheme())
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.path() == "/"
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        "APP_URL must be an HTTP(S) origin without credentials, path, query or fragment"
    );
    if std::env::var("APP_ENV").as_deref() == Ok("production") {
        anyhow::ensure!(
            parsed.scheme() == "https",
            "APP_ENV=production requires an HTTPS APP_URL"
        );
    }
    let bootstrap = std::env::var("BOOTSTRAP_TOKEN").unwrap_or_default();
    let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&db)
        .await?;
    anyhow::ensure!(
        bootstrap.is_empty() || bootstrap.len() >= 32,
        "BOOTSTRAP_TOKEN must contain at least 32 characters"
    );
    if owners == 0 && std::env::var("APP_ENV").as_deref() == Ok("production") {
        anyhow::ensure!(
            bootstrap.len() >= 32,
            "The first production owner requires a random BOOTSTRAP_TOKEN (openssl rand -hex 32)"
        );
    }
    let app = Arc::new(App {
        db,
        events: broadcast::channel(128).0,
        secure: origin.starts_with("https://"),
        origin,
        key,
        private: std::env::var("ALLOW_PRIVATE_TARGETS").as_deref() == Ok("true"),
        brand: std::env::var("APP_NAME").unwrap_or_else(|_| "Aegis Monitor".into()),
    });
    engine::start(app.clone());
    let routes = router(app.clone());
    tokio::spawn(async move {
        loop {
            if !matches!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    sqlx::query("SELECT 1").execute(&mut *leader)
                )
                .await,
                Ok(Ok(_))
            ) {
                tracing::error!("Scheduler lock connection lost; exiting to prevent split brain");
                std::process::exit(1);
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
    let bind = std::env::var("API_BIND").unwrap_or_else(|_| "0.0.0.0:3001".into());
    let bind: std::net::SocketAddr = bind.parse()?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "Aegis API listening");
    axum::serve(listener, routes)
        .with_graceful_shutdown(async {
            #[cfg(unix)]
            {
                if let Ok(mut term) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                {
                    tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
                } else {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        })
        .await?;
    Ok(())
}

pub fn router(app: Arc<App>) -> Router {
    let protected = Router::new()
        .route("/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/password", post(auth::password))
        .route("/auth/sessions", get(auth::sessions))
        .route("/auth/sessions/{id}", delete(auth::revoke_session))
        .route("/auth/refresh", post(auth::refresh_session))
        .route("/users", get(auth::users).post(auth::add_user))
        .route("/users/{id}", delete(auth::delete_user))
        .route("/monitors", get(api::monitors).post(api::create_monitor))
        .route(
            "/monitors/{id}",
            get(api::monitor)
                .put(api::update_monitor)
                .delete(api::delete_monitor),
        )
        .route("/monitors/{id}/toggle", post(api::toggle))
        .route("/monitors/{id}/checks", get(api::checks))
        .route("/monitors/{id}/uptime", get(api::uptime))
        .route("/monitors/test", post(api::test_monitor))
        .route("/monitors/summaries", get(api::summaries))
        .route("/monitors/counts", get(api::counts))
        .route("/monitors/search-count", get(api::monitor_search_count))
        .route(
            "/incidents",
            get(status::incidents).post(status::create_incident),
        )
        .route("/incidents/{id}", put(status::update_incident))
        .route("/incidents/{id}/note", put(api::annotate))
        .route(
            "/incidents/{id}/updates",
            get(status::incident_updates).post(status::add_update),
        )
        .route(
            "/resources",
            get(status::resources).post(status::create_resource),
        )
        .route(
            "/resources/{id}",
            put(status::update_resource).delete(status::delete_resource),
        )
        .route("/audit", get(status::audit_log))
        .route("/servers", get(api::servers).post(api::create_server))
        .route("/servers/{id}/metrics", get(api::server_metrics))
        .route("/servers/{id}", delete(api::delete_server))
        .route("/servers/{id}/rotate", post(api::rotate_server))
        .route(
            "/channels",
            get(notifications::channels).post(notifications::create_channel),
        )
        .route(
            "/channels/{id}",
            delete(api::delete_channel).put(notifications::update_channel),
        )
        .route("/channels/{id}/test", post(notifications::test_channel))
        .route("/deliveries", get(api::deliveries))
        .route("/channels/preview", post(notifications::preview))
        .route(
            "/deliveries/{id}/retry",
            post(notifications::retry_delivery),
        )
        .route(
            "/maintenance",
            get(api::maintenance).post(status::create_maintenance),
        )
        .route(
            "/maintenance/{id}",
            delete(api::delete_maintenance).put(status::update_maintenance),
        )
        .route("/status-pages", get(api::pages).post(status::create_page))
        .route(
            "/status-pages/{id}",
            put(status::update_page).delete(api::delete_page),
        )
        .route("/events", get(api::events))
        .route("/system", get(api::system))
        .route_layer(middleware::from_fn_with_state(app.clone(), auth::guard));
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/metrics", get(api::prometheus))
        .nest(
            "/api/v1",
            Router::new()
                .merge(protected)
                .route("/setup", get(auth::setup_state).post(auth::setup))
                .route("/auth/login", post(auth::login))
                .route("/auth/forgot", post(auth::forgot))
                .route("/public/status/{slug}", get(status::public_page))
                .route("/public/status/{slug}/feed", get(status::public_feed))
                .route("/push/{token}", post(api::push))
                .route("/agent/heartbeat", post(api::agent_heartbeat)),
        )
        .layer(DefaultBodyLimit::max(262144))
        .layer(middleware::from_fn(auth::headers))
        .with_state(app.clone())
}
#[cfg(test)]
mod integration;
