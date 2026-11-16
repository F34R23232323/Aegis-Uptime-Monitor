use crate::*;
use aegis_checks::Observation;
use serde_json::Value;
use sqlx::Row;
use std::time::Duration;
use chrono::Timelike;
use uuid::Uuid;
pub fn start(a: Arc<App>) {
    let scheduler = a.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = schedule(&scheduler).await {
                tracing::error!(error=?e,"scheduler cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    let alerts = a.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = deliver(&alerts).await {
                tracing::error!(error=?e,"notification cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    let reminders = a.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = enqueue_reminders(&reminders).await {
                tracing::error!(error=?e,"reminder scheduling failed");
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
    tokio::spawn(async move {
        loop {
            if let Err(e) = prune(&a).await {
                tracing::error!(error=?e,"retention failed");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}
async fn schedule(a: &Arc<App>) -> Result<()> {
    let workers = std::env::var("CHECK_WORKERS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(16)
        .clamp(1, 128);
    let rows=sqlx::query("UPDATE monitors SET next_check_at=now()+make_interval(secs=>interval_seconds+random()*2) WHERE id IN(SELECT id FROM monitors WHERE enabled AND next_check_at<=now() ORDER BY next_check_at LIMIT $1) RETURNING *").bind(workers).fetch_all(&a.db).await?;
    let mut tasks = tokio::task::JoinSet::new();
    for row in rows {
        let app = a.clone();
        tasks.spawn(async move {
            let id: Uuid = row.get("id");
            if let Some(observation) = evaluate(&app, &row).await? {
                record(&app, id, observation).await?;
            }
            Ok::<(), Error>(())
        });
    }
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::error!(error=?e,"Check persistence failed"),
            Err(e) => tracing::error!(error=%e,"Check task failed"),
        }
    }
    Ok(())
}
async fn evaluate(a: &App, row: &sqlx::postgres::PgRow) -> Result<Option<Observation>> {
    let mut m = api::row_input(row);
    if aegis_core::network_kind(&m.kind) {
        if crate::credentials::inject(a, &mut m, row.get("secret_config")).is_err() {
            return Ok(Some(Observation::from_error(
                "Stored monitor credentials could not be decrypted",
            )));
        }
        let retries = m.config["check_retries"].as_u64().unwrap_or(0).min(3);
        let mut observation = aegis_checks::execute(&m, a.private).await;
        for attempt in 0..retries {
            if observation.ok {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250 * (attempt + 1))).await;
            observation = aegis_checks::execute(&m, a.private).await;
        }
        return Ok(Some(observation));
    }
    if m.kind == "push" {
        let last: Option<chrono::DateTime<chrono::Utc>> = row.get("last_push");
        let created: chrono::DateTime<chrono::Utc> = row.get("created_at");
        let age = (chrono::Utc::now() - last.unwrap_or(created)).num_seconds();
        let grace = m.config["heartbeat_grace_seconds"]
            .as_i64()
            .unwrap_or(i64::from(m.interval_seconds));
        return Ok(if age <= i64::from(m.interval_seconds) + grace {
            last.map(|t| Observation {
                ok: true,
                latency_ms: 0.0,
                error: None,
                observed: json!({"last_push":t}),
            })
        } else {
            Some(Observation::from_error(
                "Expected push heartbeat was not received within interval plus grace",
            ))
        });
    }
    let id: Option<Uuid> = row.get("server_id");
    let Some(id) = id else {
        return Ok(Some(Observation::from_error(
            "Monitor requires an enrolled agent",
        )));
    };
    let row = sqlx::query(
        "SELECT inventory,last_seen>now()-interval '60 seconds' AS fresh FROM servers WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&a.db)
    .await?;
    Ok(Some(match row {
        Some(row) => api::agent_observation(
            &m,
            &row.get::<Value, _>("inventory"),
            row.get::<Option<bool>, _>("fresh").unwrap_or(false),
        ),
        None => Observation::from_error("Agent not enrolled"),
    }))
}
pub async fn record(a: &App, id: Uuid, o: Observation) -> Result<()> {
    let mut tx = a.db.begin().await?;
    let row = sqlx::query("SELECT * FROM monitors WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        return Ok(());
    };
    if !row.get::<bool, _>("enabled") {
        return Ok(());
    }
    let maintenance:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM maintenance_windows WHERE starts_at<=now() AND ends_at>now() AND $1=ANY(monitor_ids))").bind(id).fetch_one(&mut *tx).await?;
    let previous: String = row.get("state");
    let (state, failures, successes) = if maintenance {
        ("MAINTENANCE", 0, 0)
    } else {
        aegis_core::transition(
            &previous,
            o.ok,
            row.get("failures"),
            row.get("successes"),
            row.get("failure_threshold"),
            row.get("recovery_threshold"),
        )
    };
    sqlx::query("UPDATE monitors SET state=$2,failures=$3,successes=$4,last_checked_at=now(),last_latency=$5,last_error=$6 WHERE id=$1").bind(id).bind(state).bind(failures).bind(successes).bind(o.latency_ms).bind(&o.error).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO checks(monitor_id,ok,state,latency_ms,error,observed) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(o.ok).bind(state).bind(o.latency_ms).bind(&o.error).bind(o.observed).execute(&mut *tx).await?;
    let open: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM incidents WHERE monitor_id=$1 AND ended_at IS NULL")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    if !maintenance {
        if state == "DOWN" && open.is_none() {
            let incident = Uuid::new_v4();
            sqlx::query("INSERT INTO incidents(id,monitor_id,cause,title) VALUES($1,$2,$3,$4)")
                .bind(incident)
                .bind(id)
                .bind(&o.error)
                .bind(row.get::<String, _>("name"))
                .execute(&mut *tx)
                .await?;
            enqueue(
                &mut tx,
                a,
                &row,
                incident,
                "DOWN",
                o.error.as_deref().unwrap_or("Check failed"),
            )
            .await?;
        } else if state == "UP"
            && let Some(incident) = open
        {
            sqlx::query(
                "UPDATE incidents SET ended_at=now(),phase='resolved',updated_at=now() WHERE id=$1",
            )
            .bind(incident)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE notification_deliveries SET state='cancelled',last_error=NULL WHERE incident_id=$1 AND event='DOWN' AND state='pending'")
                .bind(incident).execute(&mut *tx).await?;
            enqueue(
                &mut tx,
                a,
                &row,
                incident,
                "RECOVERED",
                "Service is healthy again",
            )
            .await?;
        }
        if !o.ok
            && let Some(incident) = open
        {
            sqlx::query("INSERT INTO incident_events(incident_id,message) SELECT $1,$2 WHERE NOT EXISTS(SELECT 1 FROM incident_events WHERE incident_id=$1 AND created_at>now()-interval '5 minutes')").bind(incident).bind(o.error.as_deref().unwrap_or("Check failed")).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    let _ = a.events.send(format!("monitor:{id}"));
    Ok(())
}
async fn enqueue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    a: &App,
    row: &sqlx::postgres::PgRow,
    incident: Uuid,
    event: &str,
    reason: &str,
) -> Result<()> {
    let id: Uuid = row.get("id");
    let duration:i64=sqlx::query_scalar("SELECT greatest(0,extract(epoch FROM coalesce(ended_at,now())-started_at))::bigint FROM incidents WHERE id=$1").bind(incident).fetch_one(&mut **tx).await?;
    let elapsed = if duration < 1 {
        "Just detected".to_owned()
    } else if duration < 60 {
        format!("{duration}s")
    } else {
        format!("{}m {}s", duration / 60, duration % 60)
    };
    let channels=sqlx::query("SELECT * FROM notification_channels c WHERE enabled AND $1=ANY(events) AND (cardinality(monitor_ids)=0 OR $2=ANY(monitor_ids)) AND ($1!='DOWN' OR NOT EXISTS(SELECT 1 FROM notification_deliveries d WHERE d.channel_id=c.id AND d.event='DOWN' AND d.payload->>'monitor_id'=($2::uuid)::text AND d.created_at>now()-make_interval(secs=>c.cooldown_seconds)))").bind(event).bind(id).fetch_all(&mut **tx).await?;
    for channel in channels {
        let payload = json!({"name":row.get::<String,_>("name"),"monitor_id":id,"incident_id":incident,"state":event,"reason":reason,"timestamp":chrono::Utc::now().to_rfc3339(),"duration":elapsed,"url":format!("{}/monitors/{}",a.origin,id),"template":channel.get::<Value,_>("template")});
        sqlx::query("INSERT INTO notification_deliveries(id,channel_id,incident_id,event,payload,due_at) VALUES($1,$2,$3,$4,$5,now()+make_interval(secs=>$6)) ON CONFLICT(channel_id,incident_id,event,reminder_slot) DO NOTHING").bind(Uuid::new_v4()).bind(channel.get::<Uuid,_>("id")).bind(incident).bind(event).bind(payload).bind(if event == "DOWN" { channel.get::<i32,_>("delay_seconds") } else { 0 }).execute(&mut **tx).await?;
    }
    Ok(())
}
pub(crate) async fn enqueue_reminders(a: &App) -> Result<()> {
    sqlx::query("INSERT INTO notification_deliveries(id,channel_id,incident_id,event,payload,reminder_slot) SELECT gen_random_uuid(),c.id,i.id,'DOWN',jsonb_build_object('name',m.name,'monitor_id',m.id,'incident_id',i.id,'state','DOWN','reason',coalesce(m.last_error,'Incident remains open'),'timestamp',now(),'duration',concat(greatest(1,floor(extract(epoch from now()-i.started_at)/60)::integer),'m'),'url',concat($1::text,'/monitors/',m.id),'template',c.template,'reminder',true),floor(extract(epoch from now()-i.started_at)/c.reminder_seconds)::integer FROM incidents i JOIN monitors m ON m.id=i.monitor_id JOIN notification_channels c ON c.enabled AND 'DOWN'=ANY(c.events) AND (cardinality(c.monitor_ids)=0 OR m.id=ANY(c.monitor_ids)) WHERE i.ended_at IS NULL AND c.reminder_seconds>=300 AND now()-i.started_at>=make_interval(secs=>c.reminder_seconds) AND EXISTS(SELECT 1 FROM notification_deliveries first WHERE first.channel_id=c.id AND first.incident_id=i.id AND first.event='DOWN' AND first.reminder_slot=0 AND first.state='sent') AND NOT EXISTS(SELECT 1 FROM notification_deliveries existing WHERE existing.channel_id=c.id AND existing.incident_id=i.id AND existing.event='DOWN' AND existing.reminder_slot=floor(extract(epoch from now()-i.started_at)/c.reminder_seconds)::integer) ORDER BY i.started_at LIMIT 100 ON CONFLICT(channel_id,incident_id,event,reminder_slot) DO NOTHING")
        .bind(&a.origin).execute(&a.db).await?;
    Ok(())
}
pub(crate) async fn deliver(a: &App) -> Result<()> {
    // Single active worker; lease permits recovery after a crash.
    let jobs=sqlx::query("UPDATE notification_deliveries SET due_at=now()+interval '60 seconds' WHERE id IN(SELECT id FROM notification_deliveries WHERE state='pending' AND due_at<=now() ORDER BY due_at LIMIT 8) RETURNING *").fetch_all(&a.db).await?;
    for job in jobs {
        let id: Uuid = job.get("id");
        if job.get::<String, _>("event") == "DOWN"
            && let Some(incident_id) = job.get::<Option<Uuid>, _>("incident_id") {
            let open: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM incidents WHERE id=$1 AND ended_at IS NULL)")
                .bind(incident_id)
                .fetch_one(&a.db)
                .await?;
            if !open {
                sqlx::query("UPDATE notification_deliveries SET state='cancelled',last_error=NULL WHERE id=$1")
                    .bind(id).execute(&a.db).await?;
                continue;
            }
        }
        let channel_id: Option<Uuid> = job.get("channel_id");
        let channel = if let Some(channel_id) = channel_id {
            sqlx::query("SELECT * FROM notification_channels WHERE id=$1 AND enabled")
                .bind(channel_id)
                .fetch_optional(&a.db)
                .await?
        } else {
            None
        };
        if job.get::<String, _>("event") == "DOWN"
            && let Some(monitor_id) = job.get::<Value, _>("payload")["monitor_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()) {
                let parent_down: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM monitors child JOIN monitors parent ON parent.id=child.depends_on WHERE child.id=$1 AND parent.state='DOWN' AND parent.enabled)")
                    .bind(monitor_id).fetch_one(&a.db).await?;
                if parent_down {
                    sqlx::query("UPDATE notification_deliveries SET due_at=now()+interval '60 seconds' WHERE id=$1")
                        .bind(id).execute(&a.db).await?;
                    continue;
                }
        }
        if job.get::<String, _>("event") != "TEST"
            && let Some(ref c) = channel
            && let (Some(start), Some(end)) = (c.get::<Option<i32>, _>("quiet_start_minute"), c.get::<Option<i32>, _>("quiet_end_minute")) {
                    let now = chrono::Utc::now();
                    let minute = (now.time().num_seconds_from_midnight() / 60) as i32;
                    if let Some(wait_minutes) = quiet_minutes(start, end, minute) {
                        sqlx::query("UPDATE notification_deliveries SET due_at=now()+make_interval(secs=>$2) WHERE id=$1")
                            .bind(id).bind(wait_minutes * 60).execute(&a.db).await?;
                        continue;
                    }
        }
        let result = match channel {
            Some(c) => tokio::time::timeout(
                Duration::from_secs(15),
                send(a, &c, &job.get::<Value, _>("payload")),
            )
            .await
            .unwrap_or_else(|_| Err(bad("Provider timed out"))),
            None => Err(bad("Channel removed or disabled")),
        };
        let attempt: i32 = job.get::<i32, _>("attempts") + 1;
        let (state, error) = match result {
            Ok(()) => ("sent", None),
            Err(_) => (
                if attempt >= 8 { "failed" } else { "pending" },
                Some("Provider rejected request or could not be reached"),
            ),
        };
        sqlx::query("UPDATE notification_deliveries SET state=$2,attempts=$3,last_error=$4,due_at=now()+make_interval(secs=>$5) WHERE id=$1").bind(id).bind(state).bind(attempt).bind(error).bind((5*2_i32.pow(attempt as u32)).min(3600)).execute(&a.db).await?;
        let _ = a.events.send("deliveries".into());
    }
    Ok(())
}
fn quiet_minutes(start: i32, end: i32, now: i32) -> Option<i32> {
    if start < end {
        (now >= start && now < end).then_some(end - now)
    } else if now >= start {
        Some(1440 - now + end)
    } else if now < end {
        Some(end - now)
    } else {
        None
    }
}
async fn send(a: &App, c: &sqlx::postgres::PgRow, p: &Value) -> Result<()> {
    let endpoint = decrypt(a, &c.get::<String, _>("secret"))?;
    if c.get::<String, _>("provider") == "email" {
        return crate::notifications::send_email(a, &endpoint, p).await;
    }
    let u = reqwest::Url::parse(&endpoint).map_err(|_| bad("Invalid provider URL"))?;
    let host = u.host_str().ok_or_else(|| bad("Missing provider host"))?;
    let addresses = aegis_checks::resolve(
        host,
        u.port_or_known_default()
            .ok_or_else(|| bad("Missing port"))?,
        a.private,
    )
    .await
    .map_err(|_| bad("Provider target blocked"))?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| bad("HTTP client failure"))?;
    let payload = if c.get::<String, _>("provider") == "discord" {
        let template: crate::notifications::EmbedTemplate = serde_json::from_value(
            p.get("template")
                .cloned()
                .unwrap_or_else(|| c.get::<Value, _>("template")),
        )
        .map_err(|_| bad("Invalid stored Discord template"))?;
        crate::notifications::render(&template, p, &a.brand)?
    } else {
        p.clone()
    };
    let response = client
        .post(u)
        .json(&payload)
        .send()
        .await
        .map_err(|_| bad("Provider connection failed"))?;
    if !response.status().is_success() {
        return Err(bad("Provider delivery failed"));
    }
    Ok(())
}
async fn prune(a: &App) -> Result<()> {
    for query in [
        "DELETE FROM checks c USING monitors m WHERE c.monitor_id=m.id AND c.created_at<now()-make_interval(days=>coalesce((m.config->>'retention_days')::integer,90))",
        "DELETE FROM agent_submissions WHERE created_at<now()-interval '1 day'",
        "DELETE FROM server_metric_samples WHERE bucket<now()-interval '90 days'",
        "DELETE FROM sessions WHERE expires_at<now()",
        "DELETE FROM login_attempts WHERE window_start<now()-interval '1 day'",
        "DELETE FROM admin_audit_events WHERE created_at<now()-interval '365 days'",
        "DELETE FROM auth_audit_events WHERE created_at<now()-interval '90 days'",
        "DELETE FROM notification_deliveries WHERE created_at<now()-interval '90 days' AND state!='pending'",
        "DELETE FROM status_subscriptions WHERE NOT confirmed AND created_at<now()-interval '7 days'",
        "DELETE FROM status_subscription_deliveries WHERE created_at<now()-interval '30 days' AND state!='pending'",
    ] {
        sqlx::query(query).execute(&a.db).await?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_hours_cross_midnight_and_end_exclusively() {
        assert_eq!(quiet_minutes(22 * 60, 7 * 60, 23 * 60), Some(8 * 60));
        assert_eq!(quiet_minutes(22 * 60, 7 * 60, 6 * 60), Some(60));
        assert_eq!(quiet_minutes(22 * 60, 7 * 60, 7 * 60), None);
        assert_eq!(quiet_minutes(9 * 60, 17 * 60, 12 * 60), Some(5 * 60));
    }
    #[test]
    fn stale_agent_is_failure() {
        let m = aegis_core::MonitorInput {
            name: "test".into(),
            kind: "docker".into(),
            target: "a".into(),
            interval_seconds: 30,
            timeout_seconds: 10,
            failure_threshold: 2,
            recovery_threshold: 2,
            group_name: String::new(),
            description: String::new(),
            tags: vec![],
            public: false,
            server_id: None,
            config: json!({}),
        };
        assert!(
            !api::agent_observation(&m, &json!({"docker":[{"id":"a","healthy":true}]}), false).ok
        );
        assert!(
            api::agent_observation(&m, &json!({"docker":[{"id":"a","healthy":true}]}), true).ok
        );
        assert!(
            !api::agent_observation(&m, &json!({"docker":[{"id":"a","healthy":false}]}), true).ok
        );
    }
    #[test]
    fn stopped_pm2_process_reports_offline_reason() {
        let m = aegis_core::MonitorInput {
            name: "process".into(),
            kind: "pm2".into(),
            target: "/home/app/.pm2:2".into(),
            interval_seconds: 30,
            timeout_seconds: 10,
            failure_threshold: 2,
            recovery_threshold: 2,
            group_name: String::new(),
            description: String::new(),
            tags: vec![],
            public: false,
            server_id: None,
            config: json!({}),
        };
        let observation = api::agent_observation(
            &m,
            &json!({"pm2":[{"id":"/home/app/.pm2:2","status":"stopped","healthy":false}]}),
            true,
        );
        assert!(!observation.ok);
        assert_eq!(
            observation.error.as_deref(),
            Some("PM2 process is offline (stopped)")
        );
    }
}
