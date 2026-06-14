pub mod geometry;
pub mod gltf_import;
pub mod render;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, models::FloorplanRecord};

pub const METERS_TO_FEET: f64 = 3.280_839_895;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloorplanDocument {
    pub id: Uuid,
    pub title: String,
    pub units: String,
    pub width_ft: f64,
    pub depth_ft: f64,
    pub total_area_sqft: f64,
    pub confidence: f64,
    pub scale_label: String,
    pub rooms: Vec<Room>,
    pub walls: Vec<Wall>,
    pub openings: Vec<Opening>,
    pub furniture: Vec<Furniture>,
    pub dimensions: Vec<Dimension>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub label: String,
    pub area_sqft: f64,
    pub color: String,
    pub polygon: Vec<Point2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wall {
    pub start: Point2,
    pub end: Point2,
    pub thickness_ft: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opening {
    pub kind: String,
    pub start: Point2,
    pub end: Point2,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Furniture {
    pub kind: String,
    pub label: Option<String>,
    pub confidence: f64,
    pub rect: Rect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub label: String,
    pub start: Point2,
    pub end: Point2,
    pub offset_ft: f64,
}

pub async fn process_floorplan(state: AppState, floorplan_id: Uuid, source_bytes: Vec<u8>) {
    if let Err(err) = process_floorplan_inner(&state, floorplan_id, source_bytes).await {
        tracing::error!(%floorplan_id, error = %err, "floorplan processing failed");
        mark_failed(&state, floorplan_id, &err.to_string()).await;
    }
}

async fn process_floorplan_inner(
    state: &AppState,
    floorplan_id: Uuid,
    source_bytes: Vec<u8>,
) -> anyhow::Result<()> {
    update_job(state, floorplan_id, "processing", 8, "Loading GLB").await;

    update_job(
        state,
        floorplan_id,
        "processing",
        24,
        "Analyzing 3D geometry",
    )
    .await;
    let scan = gltf_import::scan_glb(&source_bytes)?;

    update_job(
        state,
        floorplan_id,
        "processing",
        46,
        "Detecting rooms and openings",
    )
    .await;
    let document = geometry::build_floorplan(floorplan_id, scan)?;

    update_job(
        state,
        floorplan_id,
        "processing",
        64,
        "Calculating measurements",
    )
    .await;
    let svg = render::render_svg(&document);

    update_job(
        state,
        floorplan_id,
        "processing",
        82,
        "Generating floorplan PDF",
    )
    .await;
    let pdf = render::render_pdf(&document);

    let mut jobs = state.jobs.write().await;
    if let Some(record) = jobs.get_mut(&floorplan_id) {
        record.summary.status = "complete".to_owned();
        record.summary.confidence = document.confidence;
        record.summary.total_area_sqft = Some(document.total_area_sqft);
        record.summary.width_ft = Some(document.width_ft);
        record.summary.depth_ft = Some(document.depth_ft);
        record.summary.failure_reason = None;
        record.summary.svg_url = Some(format!("/api/floorplans/{floorplan_id}/svg"));
        record.summary.pdf_url = Some(format!("/api/floorplans/{floorplan_id}/pdf"));
        record.job.status = "complete".to_owned();
        record.job.progress = 100;
        record.job.step = "Floorplan ready".to_owned();
        record.job.error = None;
        record.svg = Some(svg);
        record.pdf = Some(pdf);
    }

    Ok(())
}

async fn update_job(state: &AppState, floorplan_id: Uuid, status: &str, progress: i32, step: &str) {
    let mut jobs = state.jobs.write().await;
    if let Some(record) = jobs.get_mut(&floorplan_id) {
        record.summary.status = status.to_owned();
        record.job.status = status.to_owned();
        record.job.progress = progress;
        record.job.step = step.to_owned();
        record.job.error = None;
    }
}

async fn mark_failed(state: &AppState, floorplan_id: Uuid, message: &str) {
    let message = message.chars().take(800).collect::<String>();
    let mut jobs = state.jobs.write().await;
    if let Some(FloorplanRecord { summary, job, .. }) = jobs.get_mut(&floorplan_id) {
        summary.status = "failed".to_owned();
        summary.failure_reason = Some(message.clone());
        job.status = "failed".to_owned();
        job.progress = 100;
        job.step = "Processing failed".to_owned();
        job.error = Some(message);
    }
}
