use crate::*;
use serde_json::Value;
use sqlx::Row;
use std::time::Duration;
use uuid::Uuid;

pub fn start(a: Arc<App>) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = evaluate(&a).await {
                tracing::error!(error=?e,"SLO evaluation failed");
            }
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    });
}

pub(crate) async fn evaluate(a: &App) -> Result<()> {
    let monitors = sqlx::query("SELECT id,name,config FROM monitors WHERE enabled AND state IN ('UP','DOWN','DEGRADED') AND config ? 'slo_target_percent'")
        .fetch_all(&a.db).await?;
    for monitor in monitors {
        let id: Uuid = monitor.get("id");
        let config: Value = monitor.get("config");
        let Some(target) = config["slo_target_percent"].as_f64() else {
            continue;
        };
        let Some(days) = config["slo_window_days"].as_i64() else {
            continue;
        };
        let Some(threshold) = config["slo_alert_percent"].as_f64() else {
            continue;
        };
        let metric = api::uptime_value(a, id, days as i32).await?;
        let covered = metric["covered_seconds"].as_f64().unwrap_or(0.0);
        let down = metric["down_seconds"].as_f64().unwrap_or(0.0);
        if covered < 3600.0 {
            continue;
        }
        let budget = covered * (1.0 - target / 100.0);
        if budget <= 0.0 {
            continue;
        }
        let consumed = down / budget * 100.0;
        if !consumed.is_finite() || consumed < threshold {
            continue;
        }
        let mut tx = a.db.begin().await?;
        let channels = sqlx::query("SELECT id,template FROM notification_channels WHERE enabled AND 'SLO_BURN'=ANY(events) AND (cardinality(monitor_ids)=0 OR $1=ANY(monitor_ids))")
            .bind(id).fetch_all(&mut *tx).await?;
        if channels.is_empty() {
            continue;
        }
        let inserted: Option<Uuid> = sqlx::query_scalar("INSERT INTO slo_alerts(id,monitor_id,budget_consumed_percent) VALUES($1,$2,$3) ON CONFLICT(monitor_id,alert_date) DO NOTHING RETURNING id")
            .bind(Uuid::new_v4()).bind(id).bind(consumed).fetch_optional(&mut *tx).await?;
        if inserted.is_some() {
            for channel in channels {
                let payload = json!({"name":monitor.get::<String,_>("name"),"monitor_id":id,"state":"SLO_BURN","reason":format!("{:.1}% of the {:.3}% SLO error budget has been used over a {}-day window ({} minutes DOWN of {} measured minutes).", consumed,target,days, (down/60.0).round(),(covered/60.0).round()),"duration":format!("{days}d window"),"timestamp":chrono::Utc::now().to_rfc3339(),"url":format!("{}/monitors/{id}",a.origin),"template":channel.get::<Value,_>("template")});
                sqlx::query("INSERT INTO notification_deliveries(id,channel_id,event,payload) VALUES($1,$2,'SLO_BURN',$3)")
                    .bind(Uuid::new_v4()).bind(channel.get::<Uuid,_>("id")).bind(payload).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
    }
    Ok(())
}
