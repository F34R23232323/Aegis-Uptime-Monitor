//! Public components and editorial incidents never overwrite measured checks.
use crate::{
    auth::{User, write},
    *,
};
use axum::extract::{Extension, Path, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;
const RESOURCE_STATES: [&str; 6] = [
    "operational",
    "degraded",
    "partial_outage",
    "major_outage",
    "maintenance",
    "unknown",
];
const PHASES: [&str; 4] = ["investigating", "identified", "monitoring", "resolved"];
fn bounded(s: &str, max: usize) -> bool {
    s.len() <= max
}
fn hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|v| v.is_ascii_hexdigit())
}
fn safe_url(s: &str) -> bool {
    s.is_empty()
        || reqwest::Url::parse(s).is_ok_and(|u| {
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
}
pub async fn audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user: &User,
    action: &str,
    id: Uuid,
    details: Value,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO admin_audit_events(actor_id,action,target_id,details) VALUES($1,$2,$3,$4)",
    )
    .bind(user.id)
    .bind(action)
    .bind(id)
    .bind(details)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
async fn validate_ids(db: &sqlx::PgPool, table: &str, ids: &[Uuid]) -> Result<()> {
    if ids.len() > 200 {
        return Err(bad("At most 200 resources may be selected"));
    }
    let query = match table {
        "monitors" => "SELECT count(*) FROM monitors WHERE id=ANY($1)",
        "status_resources" => "SELECT count(*) FROM status_resources WHERE id=ANY($1)",
        _ => "SELECT count(*) FROM status_pages WHERE id=ANY($1)",
    };
    let count: i64 = sqlx::query_scalar(query).bind(ids).fetch_one(db).await?;
    if count as usize != ids.len() {
        return Err(bad("Selection contains duplicate or missing resources"));
    }
    Ok(())
}
#[derive(Deserialize)]
pub struct ResourceInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub monitor_ids: Vec<Uuid>,
    #[serde(default)]
    pub public: bool,
    pub override_state: Option<String>,
    #[serde(default)]
    pub override_reason: String,
    pub override_until: Option<DateTime<Utc>>,
}
impl ResourceInput {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty()
            || !bounded(&self.name, 120)
            || !bounded(&self.description, 2000)
            || !bounded(&self.group_name, 120)
            || !bounded(&self.override_reason, 1000)
            || self
                .override_state
                .as_deref()
                .is_some_and(|s| !RESOURCE_STATES.contains(&s))
        {
            return Err(bad("Invalid resource name, description or status"));
        }
        if self.override_until.is_some_and(|t| t <= Utc::now()) {
            return Err(bad("Override expiry must be in the future"));
        }
        if self.override_state.is_some() && self.override_reason.trim().is_empty() {
            return Err(bad("Manual status overrides require a public-safe reason"));
        }
        Ok(())
    }
}
pub async fn resources(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(r) ORDER BY r.group_name,r.name),'[]') FROM status_resources r").fetch_one(&a.db).await?))
}
pub async fn create_resource(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(r): Json<ResourceInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_resource(&a, &u, Uuid::new_v4(), r, true).await
}
pub async fn update_resource(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(r): Json<ResourceInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_resource(&a, &u, id, r, false).await
}
async fn save_resource(
    a: &App,
    u: &User,
    id: Uuid,
    r: ResourceInput,
    create: bool,
) -> Result<Json<Value>> {
    r.validate()?;
    validate_ids(&a.db, "monitors", &r.monitor_ids).await?;
    if r.public {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM monitors WHERE id=ANY($1) AND NOT public")
                .bind(&r.monitor_ids)
                .fetch_one(&a.db)
                .await?;
        if count > 0 {
            return Err(bad(
                "Make linked monitors public eligible before publishing this resource",
            ));
        }
    }
    let mut tx = a.db.begin().await?;
    if create {
        sqlx::query("INSERT INTO status_resources(id,name,description,group_name,monitor_ids,public,override_state,override_reason,override_until) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(id).bind(r.name.trim()).bind(&r.description).bind(&r.group_name).bind(&r.monitor_ids).bind(r.public).bind(&r.override_state).bind(&r.override_reason).bind(r.override_until).execute(&mut *tx).await?;
    } else {
        let count=sqlx::query("UPDATE status_resources SET name=$2,description=$3,group_name=$4,monitor_ids=$5,public=$6,override_state=$7,override_reason=$8,override_until=$9,updated_at=now() WHERE id=$1").bind(id).bind(r.name.trim()).bind(&r.description).bind(&r.group_name).bind(&r.monitor_ids).bind(r.public).bind(&r.override_state).bind(&r.override_reason).bind(r.override_until).execute(&mut *tx).await?.rows_affected();
        if count == 0 {
            return Err(Error(StatusCode::NOT_FOUND, "Resource not found".into()));
        }
    }
    sqlx::query("INSERT INTO resource_status_events(id,resource_id,state,reason,actor_id) VALUES($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(id).bind(r.override_state.as_deref().unwrap_or("automatic")).bind(&r.override_reason).bind(u.id).execute(&mut *tx).await?;
    audit(
        &mut tx,
        u,
        if create {
            "resource.created"
        } else {
            "resource.updated"
        },
        id,
        json!({"override":r.override_state,"public":r.public}),
    )
    .await?;
    tx.commit().await?;
    let _ = a.events.send("status-pages".into());
    Ok(Json(json!({"id":id})))
}
pub async fn delete_resource(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    write(&u)?;
    let mut tx = a.db.begin().await?;
    sqlx::query("UPDATE status_pages SET resource_ids=array_remove(resource_ids,$1) WHERE $1=ANY(resource_ids)").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM status_resources WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit(&mut tx, &u, "resource.deleted", id, json!({})).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct PageInput {
    #[serde(default = "default_layout")]
    pub layout: String,
    #[serde(default = "default_metrics_settings")]
    pub metrics_settings: Value,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "accent")]
    pub accent: String,
    #[serde(default)]
    pub monitor_ids: Vec<Uuid>,
    #[serde(default)]
    pub resource_ids: Vec<Uuid>,
    #[serde(default = "default_tabs")]
    pub tabs: Value,
    #[serde(default = "theme")]
    pub theme: String,
    #[serde(default)]
    pub logo_url: String,
    #[serde(default)]
    pub support_url: String,
    #[serde(default)]
    pub announcement: String,
}
fn default_layout() -> String {
    "standard".into()
}
fn default_metrics_settings() -> Value {
    json!({"show_uptime":true,"show_latency":true,"show_on_overview":true,"uptime_days":[1,7,30,90],"latency_samples":120})
}
fn accent() -> String {
    "#9a85ff".into()
}
fn theme() -> String {
    "dark".into()
}
fn default_tabs() -> Value {
    json!([{"id":"overview","label":"Overview","kind":"overview"},{"id":"incidents","label":"Incidents","kind":"incidents"},{"id":"maintenance","label":"Maintenance","kind":"maintenance"},{"id":"metrics","label":"Metrics","kind":"metrics"}])
}
fn validate_page(p: &PageInput) -> Result<()> {
    if p.slug.is_empty()
        || p.slug.len() > 80
        || !p
            .slug
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || p.title.trim().is_empty()
        || !bounded(&p.title, 120)
        || !bounded(&p.description, 2000)
        || !hex_color(&p.accent)
        || !["dark", "light", "auto"].contains(&p.theme.as_str())
        || !safe_url(&p.logo_url)
        || !safe_url(&p.support_url)
        || p.logo_url.len() > 2048
        || p.support_url.len() > 2048
        || p.announcement.len() > 1000
        || !["standard", "provider"].contains(&p.layout.as_str())
    {
        return Err(bad(
            "Invalid page title, slug, branding, theme or HTTPS URL",
        ));
    }
    let settings = p.metrics_settings.as_object().ok_or_else(|| bad("Invalid graph settings"))?;
    if !settings.keys().all(|k| ["show_uptime","show_latency","show_on_overview","uptime_days","latency_samples"].contains(&k.as_str()))
        || ["show_uptime","show_latency","show_on_overview"].iter().any(|key| !settings.get(*key).is_some_and(Value::is_boolean))
        || !settings.get("latency_samples").and_then(Value::as_u64).is_some_and(|n| [60,120,240].contains(&n))
        || !settings.get("uptime_days").and_then(Value::as_array).is_some_and(|days| !days.is_empty() && days.len() <= 4 && days.iter().all(|d| d.as_u64().is_some_and(|n| [1,7,30,90].contains(&n))))
    {
        return Err(bad("Invalid graph settings"));
    }
    let tabs = p
        .tabs
        .as_array()
        .filter(|t| !t.is_empty() && t.len() <= 8)
        .ok_or_else(|| bad("Choose between one and eight tabs"))?;
    let mut ids = std::collections::HashSet::new();
    for t in tabs {
        if !t.as_object().is_some_and(|o| {
            o.keys()
                .all(|k| ["id", "label", "kind", "body"].contains(&k.as_str()))
        }) || t.get("body").is_some_and(|b| !b.is_string())
        {
            return Err(bad(
                "Tabs may contain only id, label, kind and plain-text body",
            ));
        }
        let id = t["id"].as_str().unwrap_or("");
        let label = t["label"].as_str().unwrap_or("");
        let kind = t["kind"].as_str().unwrap_or("");
        if id.is_empty()
            || id.len() > 40
            || !id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !ids.insert(id)
            || label.trim().is_empty()
            || label.len() > 40
            || !["overview", "incidents", "maintenance", "metrics", "text"].contains(&kind)
            || t["body"].as_str().is_some_and(|s| s.len() > 8000)
        {
            return Err(bad("Invalid tab ID, label, type or text"));
        }
    }
    Ok(())
}
pub async fn create_page(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(p): Json<PageInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_page(&a, &u, Uuid::new_v4(), p, true).await
}
pub async fn update_page(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(p): Json<PageInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_page(&a, &u, id, p, false).await
}
async fn save_page(a: &App, u: &User, id: Uuid, p: PageInput, create: bool) -> Result<Json<Value>> {
    validate_page(&p)?;
    validate_ids(&a.db, "monitors", &p.monitor_ids).await?;
    validate_ids(&a.db, "status_resources", &p.resource_ids).await?;
    let mut tx = a.db.begin().await?;
    if create {
        sqlx::query("INSERT INTO status_pages(id,slug,title,description,accent,monitor_ids,resource_ids,tabs,theme,logo_url,support_url,announcement,layout,metrics_settings) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)").bind(id).bind(p.slug).bind(p.title).bind(p.description).bind(p.accent).bind(p.monitor_ids).bind(p.resource_ids).bind(p.tabs).bind(p.theme).bind(p.logo_url).bind(p.support_url).bind(p.announcement).bind(p.layout).bind(p.metrics_settings).execute(&mut *tx).await?;
    } else {
        let count=sqlx::query("UPDATE status_pages SET slug=$2,title=$3,description=$4,accent=$5,monitor_ids=$6,resource_ids=$7,tabs=$8,theme=$9,logo_url=$10,support_url=$11,announcement=$12,layout=$13,metrics_settings=$14,updated_at=now() WHERE id=$1").bind(id).bind(p.slug).bind(p.title).bind(p.description).bind(p.accent).bind(p.monitor_ids).bind(p.resource_ids).bind(p.tabs).bind(p.theme).bind(p.logo_url).bind(p.support_url).bind(p.announcement).bind(p.layout).bind(p.metrics_settings).execute(&mut *tx).await?.rows_affected();
        if count == 0 {
            return Err(Error(StatusCode::NOT_FOUND, "Status page not found".into()));
        }
    }
    audit(
        &mut tx,
        u,
        if create {
            "page.created"
        } else {
            "page.updated"
        },
        id,
        json!({}),
    )
    .await?;
    tx.commit().await?;
    let _ = a.events.send("status-pages".into());
    Ok(Json(json!({"id":id})))
}
/// Worst observed service health, with unknown evidence kept visible.
pub fn aggregate(states: &[String]) -> &'static str {
    if states.is_empty() {
        return "unknown";
    }
    let down = states.iter().filter(|s| s.as_str() == "DOWN").count();
    if down == states.len() {
        "major_outage"
    } else if down > 0 {
        "partial_outage"
    } else if states.iter().any(|s| s == "DEGRADED") {
        "degraded"
    } else if states.iter().any(|s| s == "MAINTENANCE") {
        "maintenance"
    } else if states.iter().all(|s| s == "UP") {
        "operational"
    } else {
        "unknown"
    }
}
pub async fn public_page(
    State(a): State<Arc<App>>,
    Path(slug): Path<String>,
) -> Result<Json<Value>> {
    static PUBLIC_REQUESTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
    let _permit = PUBLIC_REQUESTS.try_acquire().map_err(|_| {
        Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Status service is busy. Please retry shortly.".into(),
        )
    })?;
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        public_projection(&a, &slug),
    )
    .await
    .map_err(|_| {
        Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Status service is temporarily busy. Please retry shortly.".into(),
        )
    })?
}
pub(crate) async fn availability_batch(
    a: &App,
    ids: &[Uuid],
    days: i32,
    include_private: bool,
) -> Result<std::collections::HashMap<Uuid, Value>> {
    let rows=sqlx::query("WITH selected AS (SELECT id,interval_seconds FROM monitors WHERE id=ANY($1) AND (public OR $3)), samples AS (SELECT c.monitor_id,c.state,c.created_at,m.interval_seconds FROM checks c JOIN selected m ON m.id=c.monitor_id WHERE c.created_at>=now()-make_interval(days=>$2) UNION ALL SELECT m.id,c.state,c.created_at,m.interval_seconds FROM selected m CROSS JOIN LATERAL (SELECT state,created_at FROM checks WHERE monitor_id=m.id AND created_at<now()-make_interval(days=>$2) ORDER BY created_at DESC LIMIT 1)c), spans AS (SELECT monitor_id,state,greatest(created_at,now()-make_interval(days=>$2)) AS start,least(coalesce(lead(created_at) OVER(PARTITION BY monitor_id ORDER BY created_at),now()),created_at+make_interval(secs=>interval_seconds*2)) AS finish FROM samples) SELECT monitor_id,coalesce(sum(greatest(extract(epoch FROM finish-start),0)) FILTER(WHERE state='UP'),0)::float8 AS up,coalesce(sum(greatest(extract(epoch FROM finish-start),0)) FILTER(WHERE state='DOWN'),0)::float8 AS down FROM spans GROUP BY monitor_id").bind(ids).bind(days).bind(include_private).fetch_all(&a.db).await?;
    let mut result = std::collections::HashMap::new();
    for row in rows {
        let up: f64 = row.get("up");
        let down: f64 = row.get("down");
        result.insert(row.get("monitor_id"),json!({"days":days,"uptime":aegis_core::uptime(up,down),"covered_seconds":up+down,"up_seconds":up,"down_seconds":down}));
    }
    Ok(result)
}
fn no_coverage(days: i32) -> Value {
    json!({"days":days,"uptime":null,"covered_seconds":0.0,"up_seconds":0.0,"down_seconds":0.0})
}
async fn public_projection(a: &App, slug: &str) -> Result<Json<Value>> {
    let page = sqlx::query("SELECT * FROM status_pages WHERE slug=$1")
        .bind(slug)
        .fetch_optional(&a.db)
        .await?
        .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Status page not found".into()))?;
    let page_id: Uuid = page.get("id");
    let direct_ids: Vec<Uuid> = page.get("monitor_ids");
    let resource_ids: Vec<Uuid> = page.get("resource_ids");
    let resources = sqlx::query(
        "SELECT * FROM status_resources WHERE id=ANY($1) AND public ORDER BY array_position($1,id)",
    )
    .bind(&resource_ids)
    .fetch_all(&a.db)
    .await?;
    let mut all_ids: std::collections::HashSet<Uuid> = direct_ids.iter().copied().collect();
    for r in &resources {
        all_ids.extend(r.get::<Vec<Uuid>, _>("monitor_ids"));
    }
    if all_ids.len() > 500 {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "This status page exceeds the 500 linked-monitor limit.".into(),
        ));
    }
    let all_ids: Vec<_> = all_ids.into_iter().collect();
    let public_monitors=sqlx::query("SELECT id,name,CASE WHEN enabled AND last_checked_at<now()-make_interval(secs=>interval_seconds*2) THEN 'UNKNOWN' ELSE state END AS state FROM monitors WHERE id=ANY($1) AND public").bind(&all_ids).fetch_all(&a.db).await?;
    let states: std::collections::HashMap<Uuid, String> = public_monitors
        .iter()
        .map(|r| (r.get("id"), r.get("state")))
        .collect();
    let public_ids: Vec<Uuid> = states.keys().copied().collect();
    let mut metrics = std::collections::HashMap::new();
    for days in [1, 7, 30, 90] {
        metrics.insert(days, availability_batch(a, &public_ids, days, false).await?);
    }
    let settings: Value = page.get("metrics_settings");
    let mut latency_samples: std::collections::HashMap<Uuid, Vec<(DateTime<Utc>, f64)>> =
        std::collections::HashMap::new();
    if settings["show_latency"] == true {
        let sample_limit = settings["latency_samples"].as_i64().unwrap_or(120).clamp(60, 240);
        let rows = sqlx::query("SELECT m.id AS monitor_id,c.created_at,c.latency_ms FROM monitors m CROSS JOIN LATERAL (SELECT created_at,latency_ms FROM checks WHERE monitor_id=m.id AND ok AND latency_ms>0 ORDER BY created_at DESC LIMIT $2)c WHERE m.id=ANY($1) AND m.public ORDER BY m.id,c.created_at")
            .bind(&public_ids).bind(sample_limit).fetch_all(&a.db).await?;
        for row in rows {
            latency_samples.entry(row.get("monitor_id")).or_default().push((row.get("created_at"),row.get("latency_ms")));
        }
    }
    let summary_uptime: Vec<Value> = [1, 7, 30, 90].into_iter().map(|days| {
        let mut up = 0.0;
        let mut down = 0.0;
        for id in &public_ids {
            if let Some(value) = metrics.get(&days).and_then(|values| values.get(id)) {
                up += value["up_seconds"].as_f64().unwrap_or(0.0);
                down += value["down_seconds"].as_f64().unwrap_or(0.0);
            }
        }
        json!({"days":days,"uptime":aegis_core::uptime(up,down),"covered_seconds":up+down,"up_seconds":up,"down_seconds":down})
    }).collect();
    let mut summary_buckets: std::collections::BTreeMap<i64, (String, f64, u64)> = std::collections::BTreeMap::new();
    for samples in latency_samples.values() {
        for (at, ms) in samples {
            let entry = summary_buckets.entry(at.timestamp() / 900).or_insert_with(|| (at.to_rfc3339(), 0.0, 0));
            entry.1 += ms;
            entry.2 += 1;
        }
    }
    let summary_latency: Vec<Value> = summary_buckets.into_values().rev().take(48).map(|(at,sum,n)|json!({"at":at,"latency_ms":sum/n as f64})).collect::<Vec<_>>().into_iter().rev().collect();
    let mut resource_data = vec![];
    let mut visible_resources = vec![];
    for r in &resources {
        let id: Uuid = r.get("id");
        visible_resources.push(id);
        let ids: Vec<Uuid> = r.get("monitor_ids");
        let observed: Vec<String> = ids
            .iter()
            .filter_map(|id| states.get(id).cloned())
            .collect();
        let until: Option<DateTime<Utc>> = r.get("override_until");
        let manual: Option<String> = r.get("override_state");
        let active = manual.is_some() && until.is_none_or(|t| t > Utc::now());
        let state = if active {
            manual.as_deref().unwrap_or("unknown")
        } else if observed.len() != ids.len() {
            "unknown"
        } else {
            aggregate(&observed)
        };
        let mut windows = vec![];
        for days in [1, 7, 30, 90] {
            let mut up = 0.0;
            let mut down = 0.0;
            for id in &ids {
                if let Some(v) = metrics.get(&days).and_then(|map| map.get(id)) {
                    up += v["up_seconds"].as_f64().unwrap_or(0.0);
                    down += v["down_seconds"].as_f64().unwrap_or(0.0);
                }
            }
            windows.push(json!({"days":days,"uptime":aegis_core::uptime(up,down),"covered_seconds":up+down,"up_seconds":up,"down_seconds":down}));
        }
        let mut buckets: std::collections::BTreeMap<i64, (String, f64, u64)> = std::collections::BTreeMap::new();
        for monitor_id in &ids {
            if let Some(samples) = latency_samples.get(monitor_id) {
                for (at, ms) in samples {
                    let bucket = at.timestamp() / 900;
                    let entry = buckets.entry(bucket).or_insert_with(|| (at.to_rfc3339(), 0.0, 0));
                    entry.1 += ms;
                    entry.2 += 1;
                }
            }
        }
        let latency: Vec<Value> = buckets.into_values().rev().take(48).map(|(at,sum,n)|json!({"at":at,"latency_ms":sum/n as f64})).collect::<Vec<_>>().into_iter().rev().collect();
        resource_data.push(json!({"id":id,"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"group_name":r.get::<String,_>("group_name"),"state":state,"status_source":if active{"manual"}else{"automatic"},"override_reason":if active{r.get::<String,_>("override_reason")}else{String::new()},"override_until":if active{until}else{None},"uptime":windows,"latency":latency}));
    }
    let direct=sqlx::query("SELECT m.id,m.name,CASE WHEN m.enabled AND m.last_checked_at<now()-make_interval(secs=>m.interval_seconds*2) THEN 'UNKNOWN' ELSE m.state END AS state,coalesce((SELECT jsonb_agg(jsonb_build_object('state',c.state,'at',c.created_at) ORDER BY c.created_at) FROM (SELECT state,created_at FROM checks WHERE monitor_id=m.id ORDER BY created_at DESC LIMIT 60)c),'[]') AS history,coalesce((SELECT jsonb_agg(jsonb_build_object('started_at',i.started_at,'ended_at',i.ended_at) ORDER BY i.started_at DESC) FROM (SELECT started_at,ended_at FROM incidents WHERE monitor_id=m.id ORDER BY started_at DESC LIMIT 10)i),'[]') AS incidents FROM monitors m WHERE m.id=ANY($1) AND m.public ORDER BY array_position($1,m.id)").bind(&direct_ids).fetch_all(&a.db).await?;
    let monitors:Vec<Value>=direct.into_iter().map(|r|{let id:Uuid=r.get("id");let metric=|days|metrics.get(&days).and_then(|map|map.get(&id)).cloned().unwrap_or_else(||no_coverage(days));let latency:Vec<Value>=latency_samples.get(&id).into_iter().flatten().map(|(at,ms)|json!({"at":at,"latency_ms":ms})).collect();json!({"id":id,"name":r.get::<String,_>("name"),"state":r.get::<String,_>("state"),"history":r.get::<Value,_>("history"),"incidents":r.get::<Value,_>("incidents"),"uptime_24h":metric(1),"uptime_7d":metric(7),"uptime_30d":metric(30),"uptime_90d":metric(90),"latency":latency})}).collect();
    let incidents:Value=sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('id',i.id,'title',i.title,'severity',i.severity,'phase',i.phase,'message',i.public_message,'resource_ids',i.resource_ids,'started_at',i.started_at,'ended_at',i.ended_at,'updated_at',i.updated_at,'updates',coalesce((SELECT jsonb_agg(jsonb_build_object('id',u.id,'phase',u.phase,'message',u.message,'created_at',u.created_at) ORDER BY u.created_at DESC) FROM (SELECT id,phase,message,created_at FROM incident_updates WHERE incident_id=i.id ORDER BY created_at DESC LIMIT 100)u),'[]')) ORDER BY i.started_at DESC),'[]') FROM (SELECT * FROM incidents WHERE published AND $1=ANY(status_page_ids) AND (cardinality(resource_ids)=0 OR resource_ids && $2) ORDER BY started_at DESC LIMIT 100)i").bind(page_id).bind(&visible_resources).fetch_one(&a.db).await?;
    let maintenance:Value=sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('id',w.id,'title',w.title,'message',w.public_message,'starts_at',w.starts_at,'ends_at',w.ends_at) ORDER BY w.starts_at),'[]') FROM (SELECT * FROM maintenance_windows WHERE $1=ANY(status_page_ids) AND ends_at>now()-interval '7 days' ORDER BY starts_at LIMIT 100)w").bind(page_id).fetch_one(&a.db).await?;
    let mut overall: Vec<String> = resource_data
        .iter()
        .map(|r| r["state"].as_str().unwrap_or("unknown").to_owned())
        .collect();
    for m in &monitors {
        overall.push(
            match m["state"].as_str().unwrap_or("UNKNOWN") {
                "UP" => "operational",
                "DOWN" => "major_outage",
                "MAINTENANCE" => "maintenance",
                "DEGRADED" => "degraded",
                _ => "unknown",
            }
            .into(),
        );
    }
    Ok(Json(
        json!({"title":page.get::<String,_>("title"),"slug":slug,"description":page.get::<String,_>("description"),"accent":page.get::<String,_>("accent"),"brand":a.brand,"theme":page.get::<String,_>("theme"),"logo_url":page.get::<String,_>("logo_url"),"support_url":page.get::<String,_>("support_url"),"announcement":page.get::<String,_>("announcement"),"tabs":page.get::<Value,_>("tabs"),"resources":resource_data,"monitors":monitors,"incidents":incidents,"maintenance":maintenance,"overall_state":overall_public(&overall),"layout":page.get::<String,_>("layout"),"metrics_settings":settings,"metrics_summary":{"uptime":summary_uptime,"latency":summary_latency},"generated_at":Utc::now()}),
    ))
}

fn overall_public(states: &[String]) -> &'static str {
    for s in [
        "major_outage",
        "partial_outage",
        "degraded",
        "maintenance",
        "unknown",
    ] {
        if states.iter().any(|v| v == s) {
            return s;
        }
    }
    if states.is_empty() {
        "unknown"
    } else {
        "operational"
    }
}
#[derive(Deserialize)]
pub struct IncidentInput {
    pub title: String,
    #[serde(default = "major")]
    pub severity: String,
    #[serde(default = "investigating")]
    pub phase: String,
    #[serde(default)]
    pub public_message: String,
    #[serde(default)]
    pub published: bool,
    #[serde(default)]
    pub resource_ids: Vec<Uuid>,
    #[serde(default)]
    pub status_page_ids: Vec<Uuid>,
    #[serde(default)]
    pub note: String,
}
fn major() -> String {
    "major".into()
}
fn investigating() -> String {
    "investigating".into()
}
impl IncidentInput {
    fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty()
            || !bounded(&self.title, 180)
            || !bounded(&self.public_message, 8000)
            || !bounded(&self.note, 4000)
            || !PHASES.contains(&self.phase.as_str())
            || !["minor", "major", "critical"].contains(&self.severity.as_str())
            || self.resource_ids.len() > 200
            || self.status_page_ids.len() > 50
        {
            return Err(bad(
                "Invalid incident title, severity, phase, message or selection",
            ));
        }
        if self.published
            && (self.public_message.trim().is_empty() || self.status_page_ids.is_empty())
        {
            return Err(bad(
                "Published incidents require a public message and at least one status page",
            ));
        }
        Ok(())
    }
}
pub async fn incidents(State(a): State<Arc<App>>) -> Result<Json<Value>> {
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(i) ORDER BY i.started_at DESC),'[]') FROM (SELECT i.*,m.name AS monitor_name,(SELECT count(*) FROM incident_updates u WHERE u.incident_id=i.id) AS update_count FROM incidents i LEFT JOIN monitors m ON m.id=i.monitor_id ORDER BY i.started_at DESC LIMIT 200)i").fetch_one(&a.db).await?))
}
pub async fn create_incident(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(i): Json<IncidentInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_incident(&a, &u, Uuid::new_v4(), i, true).await
}
pub async fn update_incident(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(i): Json<IncidentInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_incident(&a, &u, id, i, false).await
}
async fn save_incident(
    a: &App,
    u: &User,
    id: Uuid,
    i: IncidentInput,
    create: bool,
) -> Result<Json<Value>> {
    i.validate()?;
    validate_ids(&a.db, "status_resources", &i.resource_ids).await?;
    validate_ids(&a.db, "status_pages", &i.status_page_ids).await?;
    let mut tx = a.db.begin().await?;
    if create {
        sqlx::query("INSERT INTO incidents(id,title,severity,phase,public_message,published,resource_ids,status_page_ids,note,ended_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,CASE WHEN $4='resolved' THEN now() ELSE NULL END)").bind(id).bind(i.title.trim()).bind(&i.severity).bind(&i.phase).bind(&i.public_message).bind(i.published).bind(&i.resource_ids).bind(&i.status_page_ids).bind(&i.note).execute(&mut *tx).await?;
    } else {
        let old = sqlx::query("SELECT monitor_id,phase FROM incidents WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Incident not found".into()))?;
        if let Some(monitor) = old.get::<Option<Uuid>, _>("monitor_id") {
            let state: Option<String> =
                sqlx::query_scalar("SELECT state FROM monitors WHERE id=$1")
                    .bind(monitor)
                    .fetch_optional(&mut *tx)
                    .await?;
            if i.phase == "resolved" && state.as_deref() != Some("UP") {
                return Err(Error(StatusCode::CONFLICT,"Automated incidents resolve after measured recovery. Change the public resource presentation separately when needed.".into()));
            }
        }
        sqlx::query("UPDATE incidents SET title=$2,severity=$3,phase=$4,public_message=$5,published=$6,resource_ids=$7,status_page_ids=$8,note=$9,ended_at=CASE WHEN $4='resolved' THEN coalesce(ended_at,now()) ELSE NULL END,updated_at=now() WHERE id=$1").bind(id).bind(i.title.trim()).bind(&i.severity).bind(&i.phase).bind(&i.public_message).bind(i.published).bind(&i.resource_ids).bind(&i.status_page_ids).bind(&i.note).execute(&mut *tx).await?;
    }
    if !i.public_message.trim().is_empty() {
        sqlx::query("INSERT INTO incident_updates(id,incident_id,phase,message,actor_id) VALUES($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(id).bind(&i.phase).bind(&i.public_message).bind(u.id).execute(&mut *tx).await?;
    }
    audit(
        &mut tx,
        u,
        if create {
            "incident.created"
        } else {
            "incident.updated"
        },
        id,
        json!({"phase":i.phase,"published":i.published}),
    )
    .await?;
    tx.commit().await?;
    let _ = a.events.send("incidents".into());
    Ok(Json(json!({"id":id})))
}
#[derive(Deserialize)]
pub struct UpdateInput {
    pub phase: String,
    pub message: String,
}
pub async fn incident_updates(
    State(a): State<Arc<App>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(u) ORDER BY u.created_at DESC),'[]') FROM incident_updates u WHERE incident_id=$1").bind(id).fetch_one(&a.db).await?))
}
pub async fn add_update(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(v): Json<UpdateInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    let row = sqlx::query("SELECT * FROM incidents WHERE id=$1")
        .bind(id)
        .fetch_optional(&a.db)
        .await?
        .ok_or_else(|| Error(StatusCode::NOT_FOUND, "Incident not found".into()))?;
    let i = IncidentInput {
        title: row.get::<String, _>("title"),
        severity: row.get("severity"),
        phase: v.phase,
        public_message: v.message,
        published: row.get("published"),
        resource_ids: row.get("resource_ids"),
        status_page_ids: row.get("status_page_ids"),
        note: row.get("note"),
    };
    save_incident(&a, &u, id, i, false).await
}
pub async fn audit_log(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
) -> Result<Json<Value>> {
    crate::auth::owner(&u)?;
    Ok(Json(sqlx::query_scalar("SELECT coalesce(jsonb_agg(to_jsonb(e) ORDER BY e.created_at DESC),'[]') FROM (SELECT a.id,a.action,a.target_id,a.details,a.created_at,u.email AS actor FROM admin_audit_events a LEFT JOIN users u ON u.id=a.actor_id ORDER BY a.created_at DESC LIMIT 200)e").fetch_one(&a.db).await?))
}
pub async fn public_feed(State(a): State<Arc<App>>, Path(slug): Path<String>) -> Result<Response> {
    let p = public_page(State(a.clone()), Path(slug.clone())).await?.0;
    fn xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
    let mut items = String::new();
    for i in p["incidents"].as_array().into_iter().flatten() {
        items.push_str(&format!("<item><guid isPermaLink=\"false\">{}</guid><title>{}</title><description>{}</description><link>{}/status/{}#incidents</link></item>",xml(i["id"].as_str().unwrap_or("")),xml(i["title"].as_str().unwrap_or("Incident")),xml(i["message"].as_str().unwrap_or("")),xml(&a.origin),xml(&slug)));
    }
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel><title>{}</title><link>{}/status/{}</link><description>{}</description>{}</channel></rss>",
        xml(p["title"].as_str().unwrap_or("Status")),
        xml(&a.origin),
        xml(&slug),
        xml(p["description"].as_str().unwrap_or("")),
        items
    );
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "application/rss+xml; charset=utf-8",
        )],
        body,
    )
        .into_response())
}
#[derive(Deserialize)]
pub struct MaintenanceInput {
    pub reason: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub monitor_ids: Vec<Uuid>,
    #[serde(default)]
    pub resource_ids: Vec<Uuid>,
    #[serde(default)]
    pub status_page_ids: Vec<Uuid>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub public_message: String,
}
pub async fn create_maintenance(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(w): Json<MaintenanceInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_maintenance(&a, &u, Uuid::new_v4(), w, true).await
}
pub async fn update_maintenance(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Path(id): Path<Uuid>,
    Json(w): Json<MaintenanceInput>,
) -> Result<Json<Value>> {
    write(&u)?;
    save_maintenance(&a, &u, id, w, false).await
}
async fn save_maintenance(
    a: &App,
    u: &User,
    id: Uuid,
    w: MaintenanceInput,
    create: bool,
) -> Result<Json<Value>> {
    if w.reason.trim().is_empty()
        || w.reason.len() > 500
        || w.ends_at <= w.starts_at
        || w.title.len() > 180
        || w.public_message.len() > 8000
        || w.status_page_ids.len() > 50
        || !w.status_page_ids.is_empty()
            && (w.title.trim().is_empty() || w.public_message.trim().is_empty())
    {
        return Err(bad(
            "Invalid maintenance dates or message; public notices require a title and public-safe description",
        ));
    }
    validate_ids(&a.db, "monitors", &w.monitor_ids).await?;
    validate_ids(&a.db, "status_resources", &w.resource_ids).await?;
    validate_ids(&a.db, "status_pages", &w.status_page_ids).await?;
    let linked: Vec<Vec<Uuid>> =
        sqlx::query_scalar("SELECT monitor_ids FROM status_resources WHERE id=ANY($1)")
            .bind(&w.resource_ids)
            .fetch_all(&a.db)
            .await?;
    let mut monitors: std::collections::BTreeSet<Uuid> = w.monitor_ids.iter().copied().collect();
    monitors.extend(linked.into_iter().flatten());
    if monitors.len() > 500 {
        return Err(bad("Maintenance may affect at most 500 monitors"));
    }
    let monitors: Vec<_> = monitors.into_iter().collect();
    let mut tx = a.db.begin().await?;
    if create {
        sqlx::query("INSERT INTO maintenance_windows(id,reason,starts_at,ends_at,monitor_ids,title,public_message,status_page_ids,resource_ids) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(id).bind(w.reason).bind(w.starts_at).bind(w.ends_at).bind(monitors).bind(w.title).bind(w.public_message).bind(w.status_page_ids).bind(w.resource_ids).execute(&mut *tx).await?;
    } else {
        let changed=sqlx::query("UPDATE maintenance_windows SET reason=$2,starts_at=$3,ends_at=$4,monitor_ids=$5,title=$6,public_message=$7,status_page_ids=$8,resource_ids=$9 WHERE id=$1").bind(id).bind(w.reason).bind(w.starts_at).bind(w.ends_at).bind(monitors).bind(w.title).bind(w.public_message).bind(w.status_page_ids).bind(w.resource_ids).execute(&mut *tx).await?.rows_affected();
        if changed == 0 {
            return Err(Error(
                StatusCode::NOT_FOUND,
                "Maintenance window not found".into(),
            ));
        }
    }
    audit(
        &mut tx,
        u,
        if create {
            "maintenance.created"
        } else {
            "maintenance.updated"
        },
        id,
        json!({}),
    )
    .await?;
    tx.commit().await?;
    let _ = a.events.send("maintenance".into());
    Ok(Json(json!({"id":id})))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn severity_and_unknown() {
        assert_eq!(aggregate(&[]), "unknown");
        assert_eq!(aggregate(&["UP".into(), "PENDING".into()]), "unknown");
        assert_eq!(aggregate(&["UP".into(), "DOWN".into()]), "partial_outage");
        assert_eq!(aggregate(&["DOWN".into()]), "major_outage");
    }
    #[test]
    fn unsafe_urls_rejected() {
        assert!(!safe_url("javascript:alert(1)"));
        assert!(!safe_url("https://user:password@example.com/"));
        assert!(safe_url("https://example.com/logo.svg"));
    }
    #[test]
    fn page_graph_settings_are_bounded() {
        let mut page: PageInput = serde_json::from_value(json!({"slug":"status","title":"Status"})).unwrap();
        assert!(validate_page(&page).is_ok());
        page.metrics_settings["latency_samples"] = json!(10000);
        assert!(validate_page(&page).is_err());
        page.metrics_settings = default_metrics_settings();
        page.metrics_settings["uptime_days"] = json!([]);
        assert!(validate_page(&page).is_err());
        page.metrics_settings = default_metrics_settings();
        page.layout = "provider".into();
        assert!(validate_page(&page).is_ok());
    }
}
