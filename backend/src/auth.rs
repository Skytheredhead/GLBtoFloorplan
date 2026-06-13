use axum::http::{HeaderMap, HeaderValue, header};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AppState,
    error::{ApiError, ApiResult},
    models::{Quota, User},
};

#[derive(Debug, Deserialize)]
struct GoogleTokenInfo {
    sub: String,
    aud: String,
    email: String,
    email_verified: Option<String>,
    name: Option<String>,
    picture: Option<String>,
}

#[derive(Debug)]
pub struct VerifiedGoogleUser {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
}

pub async fn verify_google_id_token(
    state: &AppState,
    id_token: &str,
) -> ApiResult<VerifiedGoogleUser> {
    if state.config.google_client_id.is_none() && id_token.starts_with("dev:") {
        let email = id_token.trim_start_matches("dev:").trim();
        if !email.contains('@') {
            return Err(ApiError::BadRequest("invalid dev auth email".to_owned()));
        }
        return Ok(VerifiedGoogleUser {
            sub: format!("dev-{email}"),
            email: email.to_owned(),
            name: Some("Local Demo".to_owned()),
            picture_url: None,
        });
    }

    let Some(client_id) = &state.config.google_client_id else {
        return Err(ApiError::Unauthorized);
    };

    let info = state
        .http
        .get("https://oauth2.googleapis.com/tokeninfo")
        .query(&[("id_token", id_token)])
        .send()
        .await
        .map_err(|err| ApiError::BadRequest(format!("google token check failed: {err}")))?
        .error_for_status()
        .map_err(|_| ApiError::Unauthorized)?
        .json::<GoogleTokenInfo>()
        .await
        .map_err(|err| ApiError::BadRequest(format!("google token parse failed: {err}")))?;

    if info.aud != *client_id {
        return Err(ApiError::Unauthorized);
    }
    if info.email_verified.as_deref() != Some("true") {
        return Err(ApiError::Unauthorized);
    }

    Ok(VerifiedGoogleUser {
        sub: info.sub,
        email: info.email,
        name: info.name,
        picture_url: info.picture,
    })
}

pub async fn upsert_user(state: &AppState, google: VerifiedGoogleUser) -> ApiResult<User> {
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, google_sub, email, name, picture_url)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (google_sub)
        DO UPDATE SET
          email = EXCLUDED.email,
          name = EXCLUDED.name,
          picture_url = EXCLUDED.picture_url,
          updated_at = now()
        RETURNING id, google_sub, email, name, picture_url, created_at, updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(google.sub)
    .bind(google.email)
    .bind(google.name)
    .bind(google.picture_url)
    .fetch_one(&state.pool)
    .await?;

    Ok(user)
}

pub async fn create_session(
    state: &AppState,
    user_id: Uuid,
) -> ApiResult<(String, chrono::DateTime<Utc>)> {
    let token = format!("fp_{}_{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&token);
    let expires_at = Utc::now() + Duration::days(state.config.auth_session_days);

    sqlx::query(
        r#"
        INSERT INTO user_sessions (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    Ok((token, expires_at))
}

pub async fn require_user_from_headers(state: &AppState, headers: &HeaderMap) -> ApiResult<User> {
    let token = bearer_token(headers)
        .or_else(|| cookie_token(headers, &state.config.auth_cookie_name))
        .ok_or(ApiError::Unauthorized)?;
    require_user_from_token(state, &token).await
}

pub async fn require_user_from_token(state: &AppState, token: &str) -> ApiResult<User> {
    let token_hash = hash_token(token);
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT users.id, users.google_sub, users.email, users.name, users.picture_url,
               users.created_at, users.updated_at
        FROM user_sessions
        JOIN users ON users.id = user_sessions.user_id
        WHERE user_sessions.token_hash = $1
          AND user_sessions.expires_at > now()
        "#,
    )
    .bind(token_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::Unauthorized)?;

    Ok(user)
}

pub async fn quota_for_user(state: &AppState, user_id: Uuid) -> ApiResult<Quota> {
    let month_start = current_month_start();
    let used: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::bigint
        FROM monthly_save_events
        WHERE user_id = $1 AND month_start = $2
        "#,
    )
    .bind(user_id)
    .bind(month_start)
    .fetch_one(&state.pool)
    .await?;

    let monthly_limit = state.config.monthly_free_saves;
    let remaining = (monthly_limit - used).max(0);
    Ok(Quota {
        monthly_limit,
        used,
        remaining,
        month_start,
        reset_at: next_month_start_utc(),
    })
}

pub fn auth_cookie(
    config: &crate::config::AppConfig,
    token: &str,
    expires_at: chrono::DateTime<Utc>,
) -> String {
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Expires={}; Max-Age={}",
        config.auth_cookie_name,
        token,
        expires_at.to_rfc2822(),
        config.auth_session_days * 24 * 60 * 60
    )
}

pub fn set_cookie_header(value: String) -> HeaderValue {
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
}

fn cookie_token(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == cookie_name).then(|| value.to_owned())
    })
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn current_month_start() -> NaiveDate {
    let now = Utc::now().date_naive();
    NaiveDate::from_ymd_opt(now.year(), now.month(), 1).expect("valid month")
}

fn next_month_start_utc() -> chrono::DateTime<Utc> {
    let now = Utc::now().date_naive();
    let (year, month) = if now.month() == 12 {
        (now.year() + 1, 1)
    } else {
        (now.year(), now.month() + 1)
    };
    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .expect("valid date")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_stable() {
        assert_eq!(hash_token("abc"), hash_token("abc"));
        assert_ne!(hash_token("abc"), hash_token("abcd"));
    }

    #[test]
    fn current_month_is_first_day() {
        let month = current_month_start();
        assert_eq!(month.day(), 1);
    }
}
