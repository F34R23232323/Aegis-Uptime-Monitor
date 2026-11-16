use crate::*;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::time::Duration;
use subtle::ConstantTimeEq;
use uuid::Uuid;

fn keyed_hash(a: &App, value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(a.key);
    digest.update(value.as_bytes());
    hex::encode(digest.finalize())
}

fn unsubscribe_token(a: &App, id: Uuid) -> String {
    keyed_hash(a, &format!("unsubscribe:{id}"))
}

#[cfg(test)]
pub(crate) fn test_unsubscribe_token(a: &App, id: Uuid) -> String {
    unsubscribe_token(a, id)
}

#[derive(Deserialize)]
pub struct SubscribeInput {
    email: String,
}

#[derive(Deserialize)]
pub struct TokenInput {
    id: Uuid,
    token: String,
}

pub async fn subscribe(
    State(a): State<Arc<App>>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Json(input): Json<SubscribeInput>,
) -> Result<Json<Value>> {
    auth::origin(&a, &headers)?;
    let address = input.email.trim().to_lowercase();
    if !notifications::email_address(&address) {
        return Err(bad("Enter a valid email address"));
    }
    if !std::env::var("SMTP_HOST").is_ok_and(|host| !host.trim().is_empty())
        || !std::env::var("SMTP_FROM").is_ok_and(|from| notifications::email_address(&from))
    {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Email subscriptions are not configured".into(),
        ));
    }
    auth::rate_limit(&a, "global-subscribe-budget").await?;
    auth::rate_limit(&a, &format!("subscribe:{address}")).await?;
    let page: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM status_pages WHERE slug=$1 AND subscriptions_enabled")
            .bind(&slug)
            .fetch_optional(&a.db)
            .await?;
    let page = page.ok_or_else(|| Error(StatusCode::NOT_FOUND, "Status page not found".into()))?;
    let token = secret();
    let mut tx = a.db.begin().await?;
    let id: Option<Uuid> = sqlx::query_scalar("INSERT INTO status_subscriptions(id,page_id,address_hash,address_cipher,confirm_hash) VALUES($1,$2,$3,$4,$5) ON CONFLICT(page_id,address_hash) DO UPDATE SET confirm_hash=EXCLUDED.confirm_hash,created_at=now() WHERE NOT status_subscriptions.confirmed RETURNING id")
        .bind(Uuid::new_v4()).bind(page).bind(keyed_hash(&a, &address)).bind(encrypt(&a, &address)?).bind(hash(&token))
        .fetch_optional(&mut *tx).await?;
    if let Some(id) = id {
        sqlx::query("DELETE FROM status_subscription_deliveries WHERE subscription_id=$1 AND state='pending'")
            .bind(id).execute(&mut *tx).await?;
        let payload = json!({"kind":"confirm","name":"Status page subscription","state":"CONFIRM","reason":"Confirm this email address to receive published incident updates.","duration":"","url":format!("{}/subscription#confirm={}:{}",a.origin,id,token)});
        sqlx::query("INSERT INTO status_subscription_deliveries(id,subscription_id,payload_cipher) VALUES($1,$2,$3)")
            .bind(Uuid::new_v4()).bind(id).bind(encrypt(&a, &payload.to_string())?).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(
        json!({"message":"If this address can subscribe, a confirmation email will arrive shortly."}),
    ))
}

pub async fn confirm(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<TokenInput>,
) -> Result<Json<Value>> {
    auth::origin(&a, &headers)?;
    if input.token.len() != 64 {
        return Err(bad("Invalid confirmation link"));
    }
    let changed = sqlx::query("UPDATE status_subscriptions SET confirmed=true,confirm_hash='' WHERE id=$1 AND confirm_hash=$2 AND NOT confirmed AND created_at>now()-interval '7 days'")
        .bind(input.id).bind(hash(&input.token)).execute(&a.db).await?.rows_affected();
    if changed == 0 {
        return Err(bad("Confirmation link is invalid or already used"));
    }
    Ok(Json(json!({"message":"Subscription confirmed."})))
}

pub async fn unsubscribe(
    State(a): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<TokenInput>,
) -> Result<Json<Value>> {
    auth::origin(&a, &headers)?;
    let expected = unsubscribe_token(&a, input.id);
    if !bool::from(expected.as_bytes().ct_eq(input.token.as_bytes())) {
        return Err(bad("Invalid unsubscribe link"));
    }
    sqlx::query("DELETE FROM status_subscriptions WHERE id=$1")
        .bind(input.id)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"message":"Subscription removed."})))
}

pub async fn queue_incident(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    a: &App,
    incident_id: Uuid,
    incident: &crate::status::IncidentInput,
) -> Result<()> {
    let payload = json!({"kind":"incident","name":incident.title.trim(),"state":incident.phase.to_uppercase(),"reason":incident.public_message,"duration":""});
    let cipher = encrypt(a, &payload.to_string())?;
    sqlx::query("INSERT INTO status_subscription_deliveries(id,subscription_id,incident_id,payload_cipher) SELECT gen_random_uuid(),s.id,$1,$2 FROM status_subscriptions s JOIN status_pages p ON p.id=s.page_id WHERE s.confirmed AND p.subscriptions_enabled AND s.page_id=ANY($3) AND (cardinality($4::uuid[])=0 OR p.resource_ids && $4)")
        .bind(incident_id).bind(cipher).bind(&incident.status_page_ids).bind(&incident.resource_ids).execute(&mut **tx).await?;
    Ok(())
}

pub fn start(a: Arc<App>) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = deliver(&a).await {
                tracing::error!(error=?e,"subscriber delivery failed");
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    });
}

pub(crate) async fn deliver(a: &App) -> Result<()> {
    let jobs = sqlx::query("UPDATE status_subscription_deliveries SET due_at=now()+interval '60 seconds' WHERE id IN(SELECT id FROM status_subscription_deliveries WHERE state='pending' AND due_at<=now() ORDER BY due_at LIMIT 8) RETURNING *")
        .fetch_all(&a.db).await?;
    for job in jobs {
        let id: Uuid = job.get("id");
        let recipient = sqlx::query("SELECT s.address_cipher,s.confirmed,s.page_id,p.slug,p.resource_ids,p.subscriptions_enabled FROM status_subscriptions s JOIN status_pages p ON p.id=s.page_id WHERE s.id=$1")
            .bind(job.get::<Uuid,_>("subscription_id")).fetch_optional(&a.db).await?;
        let Some(recipient) = recipient else {
            continue;
        };
        let mut payload: Value =
            serde_json::from_str(&decrypt(a, &job.get::<String, _>("payload_cipher"))?)
                .map_err(|_| bad("Invalid subscriber payload"))?;
        let confirmed: bool = recipient.get("confirmed");
        let page_enabled: bool = recipient.get("subscriptions_enabled");
        let incident_visible = if let Some(incident_id) = job.get::<Option<Uuid>, _>("incident_id")
        {
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM incidents WHERE id=$1 AND published AND $2=ANY(status_page_ids) AND (cardinality(resource_ids)=0 OR resource_ids && $3))")
                .bind(incident_id).bind(recipient.get::<Uuid,_>("page_id")).bind(recipient.get::<Vec<Uuid>,_>("resource_ids"))
                .fetch_one(&a.db).await?
        } else {
            true
        };
        if !page_enabled
            || !incident_visible
            || (payload["kind"] == "confirm" && confirmed)
            || (payload["kind"] == "incident" && !confirmed)
        {
            sqlx::query("UPDATE status_subscription_deliveries SET state='cancelled' WHERE id=$1")
                .bind(id)
                .execute(&a.db)
                .await?;
            continue;
        }
        if payload["kind"] == "incident" {
            let slug: String = recipient.get("slug");
            let subscription: Uuid = job.get("subscription_id");
            let url = format!("{}/status/{}", a.origin, slug);
            let unsubscribe = format!(
                "{}/subscription#unsubscribe={subscription}:{}",
                a.origin,
                unsubscribe_token(a, subscription)
            );
            payload["url"] = json!(url);
            payload["reason"] = json!(format!(
                "{}\n\nUnsubscribe: {}",
                payload["reason"].as_str().unwrap_or(""),
                unsubscribe
            ));
        }
        let address = decrypt(a, &recipient.get::<String, _>("address_cipher"))?;
        let result = tokio::time::timeout(
            Duration::from_secs(15),
            notifications::send_email(a, &address, &payload),
        )
        .await;
        let attempts: i32 = job.get::<i32, _>("attempts") + 1;
        match result {
            Ok(Ok(())) => {
                sqlx::query("UPDATE status_subscription_deliveries SET state='sent',attempts=$2 WHERE id=$1").bind(id).bind(attempts).execute(&a.db).await?;
            }
            _ => {
                sqlx::query("UPDATE status_subscription_deliveries SET state=CASE WHEN $2>=8 THEN 'failed' ELSE 'pending' END,attempts=$2,due_at=now()+make_interval(secs=>least(3600,30*(2^least($2,6))::integer)) WHERE id=$1").bind(id).bind(attempts).execute(&a.db).await?;
            }
        }
    }
    Ok(())
}
