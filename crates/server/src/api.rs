use crate::auth::{User, write};
use crate::*;
use aegis_checks::Observation;
use aegis_core::MonitorInput;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, header};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

fn metrics_authorized(supplied: &str, token: &str) -> bool {
    use subtle::ConstantTimeEq;
    token.len() >= 32
        && supplied.len() == token.len()
        && bool::from(hash(supplied).as_bytes().ct_eq(hash(token).as_bytes()))
}

pub async fn prometheus(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    let token = std::env::var("METRICS_TOKEN").unwrap_or_default();
    if token.len() < 32 {
        return Err(Error(StatusCode::NOT_FOUND, "Not found".into()));
    }
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");
    if !metrics_authorized(supplied, &token) {
        return Err(Error(StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }
    let counts = sqlx::query("SELECT state,count(*)::bigint AS count FROM monitors GROUP BY state")
        .fetch_all(&a.db)
        .await?;
    let mut body = String::from(
        "# HELP aegis_monitors Number of configured monitors by state.\n# TYPE aegis_monitors gauge\n",
    );
    for row in counts {
        let state: String = row.get("state");
        let count: i64 = row.get("count");
        if [
            "PENDING",
            "UP",
            "DOWN",
            "DEGRADED",
            "MAINTENANCE",
            "PAUSED",
            "UNKNOWN",
        ]
        .contains(&state.as_str())
        {
            body.push_str(&format!("aegis_monitors{{state=\"{state}\"}} {count}\n"));
        }
    }
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notification_deliveries WHERE state='pending'")
            .fetch_one(&a.db)
            .await?;
    let failed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notification_deliveries WHERE state='failed'")
            .fetch_one(&a.db)
            .await?;
    body.push_str("# HELP aegis_notification_deliveries Queued and failed notification deliveries.\n# TYPE aegis_notification_deliveries gauge\n");
    body.push_str(&format!("aegis_notification_deliveries{{state=\"pending\"}} {pending}\naegis_notification_deliveries{{state=\"failed\"}} {failed}\n"));
    Ok((
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    ))
}

#[cfg(test)]
mod metrics_tests {
    use super::metrics_authorized;
    #[test]
    fn metrics_require_strong_exact_token() {
        let token = "a".repeat(32);
        assert!(metrics_authorized(&token, &token));
        assert!(!metrics_authorized("", &token));
        assert!(!metrics_authorized(&"b".repeat(32), &token));
        assert!(!metrics_authorized("short", "short"));
    }
}

#[derive(Deserialize, Default)]
pub struct MonitorList {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub q: Option<String>,
    pub state: Option<String>,
    pub kind: Option<String>,
    pub server_id: Option<Uuid>,
}
pub async fn monitors(
    State(a): State<Arc<App>>,
    Query(q): Query<MonitorList>,
) -> Result<Json<Value>> {
    if q.q.as_ref().is_some_and(|s| s.len() > 200) {
        return Err(bad("Search query is too long"));
    }
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(m)-'push_token_hash'-'secret_config'||jsonb_build_object('has_credentials',m.secret_config IS NOT NULL) ORDER BY m.created_at DESC),'[]') FROM (SELECT * FROM monitors WHERE ($1::text IS NULL OR name ILIKE '%'||$1||'%' OR target ILIKE '%'||$1||'%' OR group_name ILIKE '%'||$1||'%') AND ($2::text IS NULL OR state=$2) AND ($3::text IS NULL OR kind=$3) AND ($4::uuid IS NULL OR server_id=$4) ORDER BY created_at DESC LIMIT $5 OFFSET $6)m").bind(q.q).bind(q.state).bind(q.kind).bind(q.server_id).bind(q.limit.unwrap_or(500).clamp(1,500)).bind(q.offset.unwrap_or(0).clamp(0,1000000)).fetch_one(&a.db).await?))
}
pub async fn monitor(State(a): State<Arc<App>>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    Ok(Json(
        sqlx::query_scalar("SELECT to_jsonb(m)-'push_token_hash'-'secret_config'||jsonb_build_object('has_credentials',m.secret_config IS NOT NULL) FROM monitors m WHERE id=$1")
            .bind(id)
            .fetch_optional(&a.db)
            .await?
            .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Monitor not found".into()))?,
    ))
}
async fn validate(a: &App, m: &MonitorInput) -> Result<Option<Uuid>> {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        aegis_checks::validate_target(m, a.private),
    )
    .await
    .map_err(|_| bad("Target validation timed out"))?
    .map_err(|e| bad(&e.to_string()))?;
    if m.config.as_object().is_some_and(|c| {
        c.keys().any(|k| {
            ![
                "method",
                "status",
                "keyword",
                "expected",
                "warning_days",
                "cpu_threshold",
                "memory_threshold",
                "record_type",
                "json_pointer",
                "json_expected",
                "json_component_id",
                "json_google_product_id",
                "disk_threshold",
                "disk_mount",
                "load_threshold",
                "heartbeat_grace_seconds",
                "check_retries",
                "follow_redirects",
                "retention_days",
            ]
            .contains(&k.as_str())
        })
    }) {
        return Err(bad(
            "Unsupported configuration key; use the separate credentials field for secrets.",
        ));
    }
    let id = m
        .server_id
        .as_ref()
        .map(|s| s.parse::<Uuid>())
        .transpose()
        .map_err(|_| bad("Invalid server ID"))?;
    if let Some(id) = id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM servers WHERE id=$1)")
            .bind(id)
            .fetch_one(&a.db)
            .await?;
        if !exists {
            return Err(bad("Server does not exist"));
        }
    }
    Ok(id)
}
pub async fn create_monitor(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(request): Json<crate::credentials::MonitorRequest>,
) -> Result<Json<Value>> {
    write(&u)?;
    let m = request.monitor;
    let encrypted = crate::credentials::protected(&a, &m, request.credentials.as_ref())?;
    let server = validate(&a, &m).await?;
    let id = Uuid::new_v4();
    let push = if m.kind == "push" {
        Some(secret())
    } else {
        None
    };
    sqlx::query("INSERT INTO monitors(id,name,kind,target,interval_seconds,timeout_seconds,failure_threshold,recovery_threshold,group_name,description,tags,public,server_id,config,push_token_hash,secret_config) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)").bind(id).bind(m.name.trim()).bind(m.kind).bind(m.target).bind(m.interval_seconds).bind(m.timeout_seconds).bind(m.failure_threshold).bind(m.recovery_threshold).bind(m.group_name).bind(m.description).bind(json!(m.tags)).bind(m.public).bind(server).bind(m.config).bind(push.as_ref().map(|s|hash(s))).bind(encrypted).execute(&a.db).await?;
    let _ = a.events.send("monitors".into());
    Ok(Json(json!({"id":id,"push_token":push})))
}
pub async fn update_monitor(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(request): Json<crate::credentials::MonitorRequest>,
) -> Result<Json<Value>> {
    write(&u)?;
    let m = request.monitor;
    let encrypted = crate::credentials::protected(&a, &m, request.credentials.as_ref())?;
    let server = validate(&a, &m).await?;
    let clear = request.credentials.as_ref().is_some_and(|c| c.clear);
    let mut tx = a.db.begin().await?;
    let old = sqlx::query("SELECT target,secret_config FROM monitors WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Monitor not found".into()))?;
    if old.get::<Option<String>, _>("secret_config").is_some()
        && encrypted.is_none()
        && !clear
        && ["http", "json"].contains(&m.kind.as_str())
    {
        let old_url = reqwest::Url::parse(&old.get::<String, _>("target"))
            .map_err(|_| bad("Stored URL is invalid"))?;
        let new_url = reqwest::Url::parse(&m.target).map_err(|_| bad("URL is invalid"))?;
        if old_url.origin() != new_url.origin() {
            return Err(bad(
                "Changing an authenticated origin requires replacing or removing the saved credentials",
            ));
        }
    }
    let rows=sqlx::query("UPDATE monitors SET name=$2,target=$3,interval_seconds=$4,timeout_seconds=$5,failure_threshold=$6,recovery_threshold=$7,group_name=$8,description=$9,tags=$10,public=$11,server_id=$12,config=$13,secret_config=CASE WHEN $16 THEN NULL ELSE coalesce($15,secret_config) END,next_check_at=now() WHERE id=$1 AND kind=$14").bind(id).bind(m.name).bind(m.target).bind(m.interval_seconds).bind(m.timeout_seconds).bind(m.failure_threshold).bind(m.recovery_threshold).bind(m.group_name).bind(m.description).bind(json!(m.tags)).bind(m.public).bind(server).bind(m.config).bind(m.kind).bind(encrypted).bind(request.credentials.as_ref().is_some_and(|c|c.clear)).execute(&mut *tx).await?.rows_affected();
    if rows == 0 {
        return Err(bad(
            "Monitor missing or type changed; create a new monitor to change type",
        ));
    }
    crate::status::audit(
        &mut tx,
        &u,
        "monitor.updated",
        id,
        json!({"credentials_changed":request.credentials.is_some(),"public":m.public}),
    )
    .await?;
    tx.commit().await?;
    let _ = a.events.send(format!("monitor:{id}"));
    Ok(Json(json!({"ok":true})))
}
pub async fn delete_monitor(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    sqlx::query("DELETE FROM monitors WHERE id=$1")
        .bind(id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn toggle(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    let mut tx = a.db.begin().await?;
    let state:Option<String>=sqlx::query_scalar("UPDATE monitors SET enabled=NOT enabled,state=CASE WHEN enabled THEN 'PAUSED' ELSE 'PENDING' END,failures=0,successes=0,next_check_at=now() WHERE id=$1 RETURNING state").bind(id).fetch_optional(&mut *tx).await?;
    let state = state.ok_or_else(|| Error(StatusCode::NOT_FOUND, "Monitor not found".into()))?;
    sqlx::query("INSERT INTO checks(monitor_id,ok,state,latency_ms) VALUES($1,false,$2,0)")
        .bind(id)
        .bind(state)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let _ = a.events.send(format!("monitor:{id}"));
    Ok(Json(json!({"ok":true})))
}
pub async fn test_monitor(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(request): Json<crate::credentials::MonitorRequest>,
) -> Result<Json<Value>> {
    write(&u)?;
    let mut m = request.monitor;
    let encrypted = crate::credentials::protected(&a, &m, request.credentials.as_ref())?;
    validate(&a, &m).await?;
    if !aegis_core::network_kind(&m.kind) {
        return Err(bad("Use the enrolled agent or push URL for this monitor"));
    }
    let encrypted = if encrypted.is_none() && request.credentials.is_none() {
        if let Some(id) = request.monitor_id {
            let row = sqlx::query("SELECT target,kind,secret_config FROM monitors WHERE id=$1")
                .bind(id)
                .fetch_optional(&a.db)
                .await?
                .ok_or_else(|| bad("Monitor not found"))?;
            if row.get::<String, _>("target") != m.target || row.get::<String, _>("kind") != m.kind
            {
                return Err(bad(
                    "Supply replacement credentials when testing a changed target",
                ));
            }
            row.get::<Option<String>, _>("secret_config")
        } else {
            None
        }
    } else {
        encrypted
    };
    crate::credentials::inject(&a, &mut m, encrypted)?;
    let r = aegis_checks::execute(&m, a.private).await;
    Ok(Json(
        json!({"ok":r.ok,"latency_ms":r.latency_ms,"error":r.error,"observed":r.observed}),
    ))
}
#[derive(Deserialize)]
pub struct History {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default = "default_days")]
    days: i32,
}
fn default_limit() -> i64 {
    120
}
fn default_days() -> i32 {
    1
}
pub async fn checks(
    State(a): State<Arc<App>>,
    Path(id): Path<Uuid>,
    Query(q): Query<History>,
) -> Result<Json<Value>> {
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(c) ORDER BY c.created_at),'[]') FROM (SELECT * FROM checks WHERE monitor_id=$1 ORDER BY created_at DESC LIMIT $2)c").bind(id).bind(q.limit.clamp(1,1000)).fetch_one(&a.db).await?))
}
/// Time weighted confirmed state, including carry-in sample. Coverage excludes unobserved gaps > 2x interval.
pub async fn uptime(
    State(a): State<Arc<App>>,
    Path(id): Path<Uuid>,
    Query(q): Query<History>,
) -> Result<Json<Value>> {
    Ok(Json(uptime_value(&a, id, q.days.clamp(1, 90)).await?))
}
pub async fn uptime_value(a: &App, id: Uuid, days: i32) -> Result<Value> {
    let row=sqlx::query("WITH samples AS (SELECT state,created_at FROM checks WHERE monitor_id=$1 AND created_at>=now()-make_interval(days=>$2) UNION ALL (SELECT state,created_at FROM checks WHERE monitor_id=$1 AND created_at<now()-make_interval(days=>$2) ORDER BY created_at DESC LIMIT 1)), spans AS (SELECT state,greatest(created_at,now()-make_interval(days=>$2)) AS start,least(coalesce(lead(created_at) OVER(ORDER BY created_at),now()),created_at+make_interval(secs=>(SELECT interval_seconds*2 FROM monitors WHERE id=$1))) AS finish FROM samples) SELECT coalesce(sum(greatest(extract(epoch FROM finish-start),0)) FILTER(WHERE state='UP'),0)::float8 AS up,coalesce(sum(greatest(extract(epoch FROM finish-start),0)) FILTER(WHERE state='DOWN'),0)::float8 AS down FROM spans").bind(id).bind(days).fetch_one(&a.db).await?;
    let up: f64 = row.get("up");
    let down: f64 = row.get("down");
    Ok(
        json!({"days":days,"uptime":aegis_core::uptime(up,down),"covered_seconds":up+down,"up_seconds":up,"down_seconds":down}),
    )
}
#[derive(Deserialize)]
pub struct Note {
    note: String,
}
pub async fn annotate(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(n): Json<Note>,
) -> Result<Json<Value>> {
    write(&u)?;
    if n.note.len() > 2000 {
        return Err(bad("Note too long"));
    }
    sqlx::query("UPDATE incidents SET note=$2 WHERE id=$1")
        .bind(id)
        .bind(n.note)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn servers(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    Ok(Json(
        sqlx::query_scalar(
            "SELECT coalesce(jsonb_agg(to_jsonb(s)-'token_hash'),'[]') FROM servers s",
        )
        .fetch_one(&a.db)
        .await?,
    ))
}
#[derive(Deserialize)]
pub struct Name {
    name: String,
}
pub async fn create_server(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(n): Json<Name>,
) -> Result<Json<Value>> {
    write(&u)?;
    if n.name.trim().is_empty() || n.name.len() > 120 {
        return Err(bad("Invalid server name"));
    }
    let id = Uuid::new_v4();
    let token = secret();
    sqlx::query("INSERT INTO servers(id,name,token_hash) VALUES($1,$2,$3)")
        .bind(id)
        .bind(n.name)
        .bind(hash(&token))
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"id":id,"token":token,"api_url":a.origin})))
}
pub async fn rotate_server(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    let token = secret();
    sqlx::query("UPDATE servers SET token_hash=$2 WHERE id=$1")
        .bind(id)
        .bind(hash(&token))
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"token":token})))
}
pub async fn delete_server(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    let used: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM monitors WHERE server_id=$1)")
        .bind(id)
        .fetch_one(&a.db)
        .await?;
    if used {
        return Err(bad("Remove this server's monitors before revoking it"));
    }
    sqlx::query("DELETE FROM servers WHERE id=$1")
        .bind(id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize, Serialize)]
pub struct AgentReport {
    pub submission_id: Uuid,
    pub version: String,
    pub inventory: Value,
}
pub async fn agent_heartbeat(
    State(a): State<Arc<App>>,
    h: HeaderMap,
    Json(report): Json<AgentReport>,
) -> Result<Json<Value>> {
    let token = h
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            Error(
                StatusCode::UNAUTHORIZED,
                "Agent credentials required".into(),
            )
        })?;
    let id: Uuid = sqlx::query_scalar("SELECT id FROM servers WHERE token_hash=$1")
        .bind(hash(token))
        .fetch_optional(&a.db)
        .await?
        .ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Invalid agent credentials".into()))?;
    if report.version.len() > 60
        || !report.inventory.is_object()
        || report.inventory.to_string().len() > 200000
    {
        return Err(bad("Agent payload too large"));
    }
    let mut tx = a.db.begin().await?;
    let inserted=sqlx::query("INSERT INTO agent_submissions(server_id,submission_id) VALUES($1,$2) ON CONFLICT DO NOTHING").bind(id).bind(report.submission_id).execute(&mut *tx).await?.rows_affected();
    if inserted == 0 {
        return Ok(Json(json!({"ok":true,"duplicate":true})));
    }
    sqlx::query("UPDATE servers SET last_seen=now(),version=$2,inventory=$3 WHERE id=$1")
        .bind(id)
        .bind(report.version)
        .bind(report.inventory)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let _ = a.events.send("servers".into());
    Ok(Json(json!({"ok":true})))
}
pub async fn push(State(a): State<Arc<App>>, Path(token): Path<String>) -> Result<Json<Value>> {
    if token.len() != 64 {
        return Err(bad("Invalid push token"));
    }
    let rows = sqlx::query(
        "UPDATE monitors SET last_push=now() WHERE push_token_hash=$1 AND kind='push' AND enabled",
    )
    .bind(hash(&token))
    .execute(&a.db)
    .await?
    .rows_affected();
    if rows == 0 {
        return Err(Error(StatusCode::NOT_FOUND, "Unknown push token".into()));
    }
    Ok(Json(json!({"ok":true})))
}
pub async fn delete_channel(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    sqlx::query("DELETE FROM notification_channels WHERE id=$1")
        .bind(id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn deliveries(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
) -> Result<Json<Value>> {
    write(&u)?;
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(d) ORDER BY created_at DESC),'[]') FROM (SELECT d.*,c.name AS channel_name FROM notification_deliveries d LEFT JOIN notification_channels c ON c.id=d.channel_id ORDER BY created_at DESC LIMIT 100)d").fetch_one(&a.db).await?))
}
pub async fn maintenance(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(w) ORDER BY starts_at DESC),'[]') FROM maintenance_windows w").fetch_one(&a.db).await?))
}
pub async fn delete_maintenance(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    sqlx::query("DELETE FROM maintenance_windows WHERE id=$1")
        .bind(id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn pages(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    Ok(Json(
        sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(p)),'[]') FROM status_pages p")
            .fetch_one(&a.db)
            .await?,
    ))
}
pub async fn delete_page(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    sqlx::query("DELETE FROM status_pages WHERE id=$1")
        .bind(id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn events(
    State(a): State<Arc<App>>,
    h: HeaderMap,
) -> axum::response::sse::Sse<
    impl futures_util::Stream<
        Item = std::result::Result<axum::response::sse::Event, std::convert::Infallible>,
    >,
> {
    let verifier = crate::auth::token(&h).map(hash).unwrap_or_default();
    let receiver = a.events.subscribe();
    let stream = futures_util::stream::unfold(
        (a, receiver, verifier, None::<std::time::Instant>),
        |(app, mut receiver, verifier, mut checked)| async move {
            loop {
                if checked.is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(15)) {
                    let valid=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM sessions WHERE token_hash=$1 AND expires_at>now())").bind(&verifier).fetch_one(&app.db).await.unwrap_or(false);
                    if !valid {
                        return None;
                    }
                    checked = Some(std::time::Instant::now());
                }
                let payload = tokio::select! {message=receiver.recv()=>match message{Ok(v)=>v,Err(tokio::sync::broadcast::error::RecvError::Lagged(_))=>"sync".into(),Err(_)=>return None},_=tokio::time::sleep(std::time::Duration::from_secs(15))=>continue};
                return Some((
                    Ok::<_, std::convert::Infallible>(
                        axum::response::sse::Event::default().data(payload),
                    ),
                    (app, receiver, verifier, checked),
                ));
            }
        },
    );
    axum::response::sse::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
pub async fn system(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    let _: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&a.db).await?;
    Ok(Json(
        json!({"version":env!("CARGO_PKG_VERSION"),"name":a.brand,"app_url":a.origin,"private_targets":a.private,"retention_days":90,"scheduler":"single active instance; PostgreSQL advisory lock","states":{"UP":"Confirmed healthy","DOWN":"Consecutive failures reached threshold","PENDING":"Waiting for enough observations","PAUSED":"Checks disabled","MAINTENANCE":"Scheduled work; alerts suppressed","UNKNOWN":"No recent observation"}}),
    ))
}
pub fn row_input(r: &sqlx::postgres::PgRow) -> MonitorInput {
    MonitorInput {
        name: r.get("name"),
        kind: r.get("kind"),
        target: r.get("target"),
        interval_seconds: r.get("interval_seconds"),
        timeout_seconds: r.get("timeout_seconds"),
        failure_threshold: r.get("failure_threshold"),
        recovery_threshold: r.get("recovery_threshold"),
        group_name: r.get("group_name"),
        description: r.get("description"),
        tags: serde_json::from_value(r.get("tags")).unwrap_or_default(),
        public: r.get("public"),
        server_id: r.get::<Option<Uuid>, _>("server_id").map(|v| v.to_string()),
        config: r.get("config"),
    }
}
pub fn agent_observation(m: &MonitorInput, inventory: &Value, fresh: bool) -> Observation {
    if !fresh {
        return Observation::from_error("Agent heartbeat missing for more than 60 seconds");
    }
    if ["host", "cpu", "memory", "disk"].contains(&m.kind.as_str()) {
        let host = &inventory["host"];
        let Some(cpu) = host["cpu_percent"].as_f64() else {
            return Observation::from_error("Agent CPU metric missing");
        };
        let Some(memory) = host["memory_percent"].as_f64() else {
            return Observation::from_error("Agent memory metric missing");
        };
        let cpu_ok = cpu < m.config["cpu_threshold"].as_f64().unwrap_or(95.0);
        let memory_ok = memory < m.config["memory_threshold"].as_f64().unwrap_or(95.0);
        let mut disk_ok = true;
        let mut disks = 0;
        for disk in host["disks"].as_array().into_iter().flatten() {
            if m.config["disk_mount"]
                .as_str()
                .filter(|s| !s.is_empty())
                .is_some_and(|mount| disk["mount"].as_str() != Some(mount))
            {
                continue;
            }
            if let (Some(total), Some(available)) = (
                disk["total"].as_f64().filter(|v| *v > 0.0),
                disk["available"].as_f64(),
            ) {
                disks += 1;
                if 100.0 * (total - available) / total
                    >= m.config["disk_threshold"].as_f64().unwrap_or(95.0)
                {
                    disk_ok = false;
                }
            }
        }
        if m.kind == "disk" && disks == 0 {
            return Observation::from_error("No matching disk metric was reported by the agent");
        }
        let load_ok = m.config["load_threshold"]
            .as_f64()
            .is_none_or(|limit| host["load"].as_f64().is_some_and(|n| n < limit));
        let ok = match m.kind.as_str() {
            "cpu" => cpu_ok,
            "memory" => memory_ok,
            "disk" => disk_ok,
            _ => {
                cpu_ok
                    && memory_ok
                    && load_ok
                    && (!m.config.get("disk_threshold").is_some() || disk_ok && disks > 0)
            }
        };
        return Observation {
            ok,
            latency_ms: 0.0,
            error: if ok {
                None
            } else {
                Some(format!("{} resource threshold exceeded", m.kind))
            },
            observed: host.clone(),
        };
    }
    let key = if m.kind == "docker" { "docker" } else { "pm2" };
    let item = inventory[key]
        .as_array()
        .and_then(|items| items.iter().find(|i| i["id"].as_str() == Some(&m.target)));
    match item {
        Some(v) => {
            let ok = v["healthy"].as_bool() == Some(true);
            let reason = if ok {
                None
            } else if m.kind == "pm2" {
                Some(match v["status"].as_str() {
                    Some("stopped") => "PM2 process is offline (stopped)".to_owned(),
                    Some("errored") => "PM2 process is offline (errored)".to_owned(),
                    Some("launching") => "PM2 process is launching".to_owned(),
                    Some(_) => "PM2 process is not online".to_owned(),
                    None => "PM2 process status is unavailable".to_owned(),
                })
            } else {
                Some("Agent reports service stopped, errored or unhealthy".to_owned())
            };
            Observation {
                ok,
                latency_ms: 0.0,
                error: reason,
                observed: v.clone(),
            }
        }
        None => Observation::from_error(
            "Service absent from agent inventory; inspect adapter configuration",
        ),
    }
}

pub async fn summaries(State(a): State<Arc<App>>, Query(q): Query<History>) -> Result<Json<Value>> {
    let rows=sqlx::query("SELECT m.id,coalesce((SELECT jsonb_agg(jsonb_build_object('id',c.id,'state',c.state,'ok',c.ok,'latency_ms',c.latency_ms,'error',c.error,'created_at',c.created_at,'observed','{}'::jsonb) ORDER BY c.created_at) FROM (SELECT id,state,ok,latency_ms,error,created_at FROM checks WHERE monitor_id=m.id ORDER BY created_at DESC LIMIT 60)c),'[]') AS checks FROM monitors m ORDER BY m.created_at DESC LIMIT 100").fetch_all(&a.db).await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.get("id")).collect();
    let days = q.days.clamp(1, 90);
    let metrics = crate::status::availability_batch(&a, &ids, days, true).await?;
    let values:Vec<_>=rows.into_iter().map(|r|{let id:Uuid=r.get("id");json!({"id":id,"checks":r.get::<Value,_>("checks"),"uptime":metrics.get(&id).cloned().unwrap_or_else(||json!({"days":days,"uptime":null,"covered_seconds":0.0,"up_seconds":0.0,"down_seconds":0.0}))})}).collect();
    Ok(Json(json!(values)))
}
pub async fn counts(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    let value:Value=sqlx::query_scalar("SELECT jsonb_build_object('total',(SELECT count(*) FROM monitors),'states',coalesce((SELECT jsonb_object_agg(state,n) FROM (SELECT CASE WHEN enabled AND last_checked_at<now()-make_interval(secs=>interval_seconds*2) THEN 'UNKNOWN' ELSE state END AS state,count(*) AS n FROM monitors GROUP BY 1)s),'{}'),'open_incidents',(SELECT count(*) FROM incidents WHERE ended_at IS NULL))").fetch_one(&a.db).await?;
    Ok(Json(value))
}
