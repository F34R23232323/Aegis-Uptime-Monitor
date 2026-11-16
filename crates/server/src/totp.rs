use crate::auth::User;
use crate::*;
use argon2::{Argon2, PasswordVerifier, password_hash::PasswordHash};
use axum::extract::{Extension, State};
use rand::RngCore;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use subtle::ConstantTimeEq;
use uuid::Uuid;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

fn base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut bits = 0u32;
    let mut count = 0;
    for &byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        count += 8;
        while count >= 5 {
            count -= 5;
            out.push(ALPHABET[((bits >> count) & 31) as usize] as char);
        }
    }
    if count > 0 {
        out.push(ALPHABET[((bits << (5 - count)) & 31) as usize] as char);
    }
    out
}

pub(crate) fn code(secret: &[u8], step: i64) -> String {
    let mut key = [0u8; 64];
    key[..secret.len()].copy_from_slice(secret);
    let mut inner = [0x36u8; 64];
    let mut outer = [0x5cu8; 64];
    for i in 0..64 {
        inner[i] ^= key[i];
        outer[i] ^= key[i];
    }
    let mut hash = Sha256::new();
    hash.update(inner);
    hash.update(step.to_be_bytes());
    let inner_hash = hash.finalize();
    let mut hash = Sha256::new();
    hash.update(outer);
    hash.update(inner_hash);
    let digest = hash.finalize();
    let offset = usize::from(digest[31] & 15);
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", value % 1_000_000)
}

fn matching_step(secret: &[u8], input: &str, last: i64) -> Option<i64> {
    if input.len() != 6 || !input.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let current = chrono::Utc::now().timestamp() / 30;
    [current - 1, current, current + 1]
        .into_iter()
        .find(|&step| {
            step > last && bool::from(code(secret, step).as_bytes().ct_eq(input.as_bytes()))
        })
}

fn secret_bytes(a: &App, encrypted: &str) -> Result<Vec<u8>> {
    let bytes =
        hex::decode(decrypt(a, encrypted)?).map_err(|_| bad("Invalid verification setup"))?;
    if bytes.len() != 32 {
        return Err(bad("Invalid verification setup"));
    }
    Ok(bytes)
}

pub async fn verify_login(a: &App, user_id: Uuid, input: Option<&str>) -> Result<()> {
    let mut tx = a.db.begin().await?;
    let row = sqlx::query("SELECT totp_secret,totp_last_step FROM users WHERE id=$1 FOR UPDATE")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
    if let Some(encrypted) = row.get::<Option<String>, _>("totp_secret") {
        let bytes = secret_bytes(a, &encrypted)?;
        let step = input
            .and_then(|code| matching_step(&bytes, code, row.get("totp_last_step")))
            .ok_or_else(|| {
                Error(
                    axum::http::StatusCode::UNAUTHORIZED,
                    "Invalid credentials or verification code".into(),
                )
            })?;
        sqlx::query("UPDATE users SET totp_last_step=$2 WHERE id=$1")
            .bind(user_id)
            .bind(step)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct PasswordInput {
    password: String,
}

#[derive(Deserialize)]
pub struct CodeInput {
    code: String,
}

#[derive(Deserialize)]
pub struct DisableInput {
    password: String,
    code: String,
}

async fn check_password(a: &App, user_id: Uuid, password: String) -> Result<()> {
    let _permit = crate::auth::password_slot().await?;
    let stored: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_one(&a.db)
        .await?;
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&stored).ok().is_some_and(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    if !valid {
        return Err(bad("Current password is incorrect"));
    }
    Ok(())
}

pub async fn state(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
) -> Result<Json<Value>> {
    let enabled: bool = sqlx::query_scalar("SELECT totp_secret IS NOT NULL FROM users WHERE id=$1")
        .bind(u.id)
        .fetch_one(&a.db)
        .await?;
    Ok(Json(json!({"enabled":enabled})))
}

pub async fn setup(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(input): Json<PasswordInput>,
) -> Result<Json<Value>> {
    check_password(&a, u.id, input.password).await?;
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let secret = base32(&bytes);
    let changed =
        sqlx::query("UPDATE users SET totp_pending_secret=$2 WHERE id=$1 AND totp_secret IS NULL")
            .bind(u.id)
            .bind(encrypt(&a, &hex::encode(bytes))?)
            .execute(&a.db)
            .await?
            .rows_affected();
    if changed == 0 {
        return Err(bad(
            "Disable existing verification before setting up a new authenticator",
        ));
    }
    let mut uri =
        reqwest::Url::parse("otpauth://totp/").map_err(|_| bad("Invalid provisioning URI"))?;
    uri.path_segments_mut()
        .map_err(|_| bad("Invalid provisioning URI"))?
        .push(&format!("Aegis Monitor:{}", u.email));
    uri.query_pairs_mut()
        .append_pair("secret", &secret)
        .append_pair("issuer", "Aegis Monitor")
        .append_pair("algorithm", "SHA256")
        .append_pair("digits", "6")
        .append_pair("period", "30");
    Ok(Json(json!({"secret":secret,"uri":uri.as_str()})))
}

pub async fn enable(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(input): Json<CodeInput>,
) -> Result<Json<Value>> {
    let mut tx = a.db.begin().await?;
    let row = sqlx::query("SELECT totp_pending_secret FROM users WHERE id=$1 FOR UPDATE")
        .bind(u.id)
        .fetch_one(&mut *tx)
        .await?;
    let encrypted: String = row
        .get::<Option<String>, _>("totp_pending_secret")
        .ok_or_else(|| bad("Start verification setup first"))?;
    let bytes = secret_bytes(&a, &encrypted)?;
    let step =
        matching_step(&bytes, &input.code, -1).ok_or_else(|| bad("Invalid verification code"))?;
    sqlx::query(
        "UPDATE users SET totp_secret=$2,totp_pending_secret=NULL,totp_last_step=$3 WHERE id=$1",
    )
    .bind(u.id)
    .bind(encrypted)
    .bind(step)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"enabled":true})))
}

pub async fn disable(
    State(a): State<Arc<App>>,
    Extension(u): Extension<User>,
    Json(input): Json<DisableInput>,
) -> Result<Json<Value>> {
    check_password(&a, u.id, input.password).await?;
    let mut tx = a.db.begin().await?;
    let row = sqlx::query("SELECT totp_secret,totp_last_step FROM users WHERE id=$1 FOR UPDATE")
        .bind(u.id)
        .fetch_one(&mut *tx)
        .await?;
    let encrypted: String = row
        .get::<Option<String>, _>("totp_secret")
        .ok_or_else(|| bad("Verification is not enabled"))?;
    let bytes = secret_bytes(&a, &encrypted)?;
    matching_step(&bytes, &input.code, row.get("totp_last_step"))
        .ok_or_else(|| bad("Invalid verification code"))?;
    sqlx::query(
        "UPDATE users SET totp_secret=NULL,totp_pending_secret=NULL,totp_last_step=-1 WHERE id=$1",
    )
    .bind(u.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({"enabled":false})))
}

#[cfg(test)]
mod tests {
    #[test]
    fn rfc_sha256_vector() {
        let secret = b"12345678901234567890123456789012";
        assert_eq!(super::code(secret, 59 / 30), "119246");
    }
}
