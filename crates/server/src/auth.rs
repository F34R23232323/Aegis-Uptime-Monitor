use crate::*;
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{
    extract::{Extension, Request, State},
    http::{HeaderMap, header},
    middleware::Next,
};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;
static PASSWORD_WORKERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
async fn password_slot() -> Result<tokio::sync::SemaphorePermit<'static>> {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        PASSWORD_WORKERS.acquire(),
    )
    .await
    .map_err(|_| {
        Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Authentication is busy. Try again shortly.".into(),
        )
    })?
    .map_err(|_| bad("Authentication worker unavailable"))
}
#[derive(Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}
pub fn write(u: &User) -> Result<()> {
    if aegis_core::can_write(&u.role) {
        Ok(())
    } else {
        Err(Error(
            StatusCode::FORBIDDEN,
            "Admin permission required".into(),
        ))
    }
}
pub fn owner(u: &User) -> Result<()> {
    if u.role == "owner" {
        Ok(())
    } else {
        Err(Error(
            StatusCode::FORBIDDEN,
            "Owner permission required".into(),
        ))
    }
}
pub async fn headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    for (k, v) in [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "same-origin"),
        ("cache-control", "no-store"),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=()",
        ),
        ("cross-origin-opener-policy", "same-origin"),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(k),
            axum::http::HeaderValue::from_static(v),
        );
    }
    response
}
pub fn origin(app: &App, h: &HeaderMap) -> Result<()> {
    if h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(app.origin.as_str()) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "Request origin does not match APP_URL".into(),
        ));
    }
    Ok(())
}
pub(crate) fn token(h: &HeaderMap) -> Option<&str> {
    h.get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|v| v.strip_prefix("aegis_session="))
}
pub async fn guard(State(app): State<Arc<App>>, mut req: Request, next: Next) -> Result<Response> {
    if !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) {
        origin(&app, req.headers())?;
    }
    let tok = token(req.headers())
        .ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Please sign in".into()))?;
    let row=sqlx::query("SELECT u.id,u.email,u.role FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.expires_at>now()").bind(hash(tok)).fetch_optional(&app.db).await?.ok_or_else(||Error(StatusCode::UNAUTHORIZED,"Session expired".into()))?;
    req.extensions_mut().insert(User {
        id: row.get("id"),
        email: row.get("email"),
        role: row.get("role"),
    });
    Ok(next.run(req).await)
}
#[derive(Deserialize)]
pub struct Credentials {
    email: String,
    password: String,
    setup_token: Option<String>,
}
fn validate(c: &Credentials) -> Result<()> {
    if c.email.len() > 254
        || !c.email.contains('@')
        || c.password.len() < 12
        || c.password.len() > 256
    {
        return Err(bad(
            "Use a valid email and a password between 12 and 256 characters",
        ));
    }
    Ok(())
}
async fn password_hash(password: String) -> Result<String> {
    let _permit = password_slot().await?;
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|h| h.to_string())
    })
    .await
    .map_err(|_| bad("Password worker failed"))?
    .map_err(|_| bad("Password hashing failed"))
}
pub async fn setup_state(State(app): State<Arc<App>>) -> Result<Json<serde_json::Value>> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&app.db)
        .await?;
    Ok(Json(
        json!({"needs_setup":count==0,"name":app.brand,"accent":crate::app_accent(),"requires_setup_token":std::env::var("BOOTSTRAP_TOKEN").is_ok_and(|t|!t.is_empty())}),
    ))
}
pub async fn setup(
    State(app): State<Arc<App>>,
    h: HeaderMap,
    Json(c): Json<Credentials>,
) -> Result<Response> {
    origin(&app, &h)?;
    let expected = std::env::var("BOOTSTRAP_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    if !bootstrap_authorized(expected.as_deref(), c.setup_token.as_deref().unwrap_or("")) {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "A valid bootstrap token is required to create the first owner".into(),
        ));
    }
    validate(&c)?;
    rate_limit(&app, "setup").await?;
    let hashed = password_hash(c.password).await?;
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(871222)")
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&mut *tx)
        .await?;
    if count != 0 {
        return Err(Error(
            StatusCode::CONFLICT,
            "Setup has already been completed".into(),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,password_hash,role) VALUES($1,$2,$3,'owner')")
        .bind(id)
        .bind(c.email.to_lowercase().trim())
        .bind(hashed)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    session(&app, id).await
}
async fn rate_limit(app: &App, email: &str) -> Result<()> {
    let attempts:i32=sqlx::query_scalar("INSERT INTO login_attempts(key,attempts) VALUES($1,1) ON CONFLICT(key) DO UPDATE SET attempts=CASE WHEN login_attempts.window_start<now()-interval '15 minutes' THEN 1 ELSE login_attempts.attempts+1 END, window_start=CASE WHEN login_attempts.window_start<now()-interval '15 minutes' THEN now() ELSE login_attempts.window_start END RETURNING attempts").bind(hash(email)).fetch_one(&app.db).await?;
    if attempts
        > if email == "global-login-budget" {
            200
        } else {
            10
        }
    {
        Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many attempts. Try again in 15 minutes".into(),
        ))
    } else {
        Ok(())
    }
}
pub async fn login(
    State(app): State<Arc<App>>,
    h: HeaderMap,
    Json(c): Json<Credentials>,
) -> Result<Response> {
    origin(&app, &h)?;
    if c.email.len() > 254 || c.password.len() > 256 {
        return Err(bad("Invalid credentials"));
    }
    rate_limit(&app, "global-login-budget").await?;
    let email = c.email.to_lowercase();
    rate_limit(&app, &email).await?;
    let row = sqlx::query("SELECT id,password_hash FROM users WHERE email=$1")
        .bind(email.trim())
        .fetch_optional(&app.db)
        .await?;
    let stored = row.as_ref().map(|r| r.get::<String, _>("password_hash"));
    let _permit = password_slot().await?;
    let valid=tokio::task::spawn_blocking(move||{let fallback="$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0c2FsdA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";PasswordHash::new(stored.as_deref().unwrap_or(fallback)).ok().is_some_and(|h|Argon2::default().verify_password(c.password.as_bytes(),&h).is_ok())}).await.unwrap_or(false);
    if !valid || row.is_none() {
        sqlx::query("INSERT INTO auth_audit_events(event) VALUES('login_failed')")
            .execute(&app.db)
            .await?;
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "Invalid email or password".into(),
        ));
    }
    let id: Uuid = row.ok_or_else(|| bad("Invalid credentials"))?.get("id");
    session(&app, id).await
}
async fn session(app: &App, id: Uuid) -> Result<Response> {
    let tok = secret();
    sqlx::query(
        "INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '7 days')",
    )
    .bind(hash(&tok))
    .bind(id)
    .execute(&app.db)
    .await?;
    sqlx::query("INSERT INTO auth_audit_events(event,user_id) VALUES('login_success',$1)")
        .bind(id)
        .execute(&app.db)
        .await?;
    let cookie = format!(
        "aegis_session={tok}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{}",
        if app.secure { "; Secure" } else { "" }
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"ok":true}))).into_response())
}
pub async fn me(Extension(u): Extension<User>) -> Json<serde_json::Value> {
    Json(json!({"id":u.id,"email":u.email,"role":u.role}))
}
pub async fn logout(State(app): State<Arc<App>>, h: HeaderMap) -> Result<Response> {
    if let Some(tok) = token(&h) {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(hash(tok))
            .execute(&app.db)
            .await?;
    }
    Ok((
        [(
            header::SET_COOKIE,
            "aegis_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    )
        .into_response())
}
#[derive(Deserialize)]
pub struct Change {
    current: String,
    password: String,
}
pub async fn password(
    State(app): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(c): Json<Change>,
) -> Result<Json<serde_json::Value>> {
    validate(&Credentials {
        email: u.email.clone(),
        password: c.password.clone(),
        setup_token: None,
    })?;
    let stored: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(u.id)
        .fetch_one(&app.db)
        .await?;
    if c.current.len() > 256 {
        return Err(bad("Invalid current password"));
    }
    let permit = password_slot().await?;
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&stored).ok().is_some_and(|h| {
            Argon2::default()
                .verify_password(c.current.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    drop(permit);
    if !valid {
        return Err(bad("Current password is incorrect"));
    }
    let hashed = password_hash(c.password).await?;
    let mut tx = app.db.begin().await?;
    sqlx::query("UPDATE users SET password_hash=$1 WHERE id=$2")
        .bind(hashed)
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(
        json!({"ok":true,"message":"Password changed. Sign in again."}),
    ))
}
pub async fn forgot(State(app): State<Arc<App>>, h: HeaderMap) -> Result<Json<serde_json::Value>> {
    origin(&app, &h)?;
    Ok(Json(
        json!({"message":"Ask the instance owner to use the documented CLI password recovery procedure. Email recovery is not configured."}),
    ))
}
pub async fn users(
    State(app): State<Arc<App>>,
    Extension(u): Extension<User>,
) -> Result<Json<serde_json::Value>> {
    owner(&u)?;
    Ok(Json(sqlx::query_scalar::<_,serde_json::Value>("SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'email',email,'role',role)),'[]') FROM users").fetch_one(&app.db).await?))
}
#[derive(Deserialize)]
pub struct NewUser {
    email: String,
    password: String,
    role: String,
}
pub async fn add_user(
    State(app): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(c): Json<NewUser>,
) -> Result<Json<serde_json::Value>> {
    owner(&u)?;
    if !["admin", "viewer"].contains(&c.role.as_str()) {
        return Err(bad("Choose admin or viewer"));
    }
    validate(&Credentials {
        email: c.email.clone(),
        password: c.password.clone(),
        setup_token: None,
    })?;
    let hashed = password_hash(c.password).await?;
    sqlx::query("INSERT INTO users(id,email,password_hash,role) VALUES($1,$2,$3,$4)")
        .bind(Uuid::new_v4())
        .bind(c.email.to_lowercase().trim())
        .bind(hashed)
        .bind(c.role)
        .execute(&app.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn delete_user(
    State(app): State<Arc<App>>,
    Extension(u): Extension<User>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    owner(&u)?;
    sqlx::query("DELETE FROM users WHERE id=$1 AND role!='owner'")
        .bind(id)
        .execute(&app.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn recover(db: &sqlx::PgPool) -> anyhow::Result<()> {
    let email = std::env::var("RECOVERY_EMAIL")?;
    let password = std::env::var("RECOVERY_PASSWORD")?;
    anyhow::ensure!(
        (12..=256).contains(&password.len()),
        "Password must be 12–256 characters"
    );
    let hashed = password_hash(password)
        .await
        .map_err(|_| anyhow::anyhow!("Hash failed"))?;
    let mut tx = db.begin().await?;
    let id: Uuid =
        sqlx::query_scalar("UPDATE users SET password_hash=$1 WHERE email=$2 RETURNING id")
            .bind(hashed)
            .bind(email.to_lowercase())
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO auth_audit_events(event,user_id) VALUES('cli_password_recovery',$1)")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    println!("Password updated; all sessions revoked.");
    Ok(())
}

pub async fn sessions(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    h: HeaderMap,
) -> Result<Json<serde_json::Value>> {
    let current = token(&h).map(hash).unwrap_or_default();
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'created_at',created_at,'expires_at',expires_at,'current',token_hash=$2) ORDER BY created_at DESC),'[]') FROM sessions WHERE user_id=$1 AND expires_at>now()").bind(u.id).bind(current).fetch_one(&a.db).await?))
}
pub async fn revoke_session(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let changed = sqlx::query("DELETE FROM sessions WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(u.id)
        .execute(&a.db)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(Error(StatusCode::NOT_FOUND, "Session not found".into()));
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn refresh_session(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    h: HeaderMap,
) -> Result<Response> {
    let old = token(&h)
        .map(hash)
        .ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Session required".into()))?;
    let token = secret();
    let mut tx = a.db.begin().await?;
    let removed =
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1 AND user_id=$2 AND expires_at>now()")
            .bind(old)
            .bind(u.id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    if removed == 0 {
        return Err(Error(StatusCode::UNAUTHORIZED, "Session expired".into()));
    }
    sqlx::query(
        "INSERT INTO sessions(token_hash,user_id,expires_at) VALUES($1,$2,now()+interval '7 days')",
    )
    .bind(hash(&token))
    .bind(u.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO auth_audit_events(event,user_id) VALUES('session_rotated',$1)")
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let cookie = format!(
        "aegis_session={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{}",
        if a.secure { "; Secure" } else { "" }
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"ok":true}))).into_response())
}

fn bootstrap_authorized(expected: Option<&str>, provided: &str) -> bool {
    use subtle::ConstantTimeEq;
    expected.is_none_or(|expected| {
        bool::from(hash(expected).as_bytes().ct_eq(hash(provided).as_bytes()))
    })
}
#[cfg(test)]
mod bootstrap_tests {
    use super::*;
    #[test]
    fn requires_exact_secret_when_configured() {
        assert!(bootstrap_authorized(None, ""));
        assert!(!bootstrap_authorized(Some("one-time-secret"), ""));
        assert!(!bootstrap_authorized(Some("one-time-secret"), "wrong"));
        assert!(bootstrap_authorized(
            Some("one-time-secret"),
            "one-time-secret"
        ));
    }
}
