use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub google_sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicUser {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
}

impl From<User> for PublicUser {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            name: user.name,
            picture_url: user.picture_url,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Quota {
    pub monthly_limit: i64,
    pub used: i64,
    pub remaining: i64,
    pub month_start: NaiveDate,
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: PublicUser,
    pub quota: Quota,
}

#[derive(Debug, Deserialize)]
pub struct GoogleAuthRequest {
    pub id_token: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: PublicUser,
    pub quota: Quota,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FloorplanRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub status: String,
    pub source_filename: String,
    pub source_size_bytes: i64,
    pub source_sha256: String,
    pub source_artifact_path: String,
    pub floorplan_json_path: Option<String>,
    pub svg_path: Option<String>,
    pub pdf_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub confidence: f64,
    pub total_area_sqft: Option<f64>,
    pub width_ft: Option<f64>,
    pub depth_ft: Option<f64>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProcessingJobRow {
    pub id: Uuid,
    pub floorplan_id: Uuid,
    pub status: String,
    pub progress: i32,
    pub step: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FloorplanSummary {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub source_filename: String,
    pub source_size_bytes: i64,
    pub confidence: f64,
    pub total_area_sqft: Option<f64>,
    pub width_ft: Option<f64>,
    pub depth_ft: Option<f64>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub svg_url: Option<String>,
    pub pdf_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FloorplanDetail {
    pub floorplan: FloorplanSummary,
    pub job: Option<JobSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct JobSnapshot {
    pub floorplan_id: Uuid,
    pub status: String,
    pub progress: i32,
    pub step: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub floorplan: FloorplanSummary,
    pub job: JobSnapshot,
}
