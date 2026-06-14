use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct Quota {
    pub daily_limit: i64,
    pub used: i64,
    pub remaining: i64,
    pub day: NaiveDate,
    pub reset_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct FloorplanDetail {
    pub floorplan: FloorplanSummary,
    pub job: Option<JobSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobSnapshot {
    pub floorplan_id: Uuid,
    pub status: String,
    pub progress: i32,
    pub step: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadResponse {
    pub floorplan: FloorplanSummary,
    pub job: JobSnapshot,
    pub quota: Quota,
}

#[derive(Debug, Clone)]
pub struct FloorplanRecord {
    pub summary: FloorplanSummary,
    pub job: JobSnapshot,
    pub svg: Option<String>,
    pub pdf: Option<Vec<u8>>,
}
