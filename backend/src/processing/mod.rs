pub mod geometry;
pub mod gltf_import;
pub mod render;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, auth, storage};

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

pub async fn process_floorplan(state: AppState, floorplan_id: Uuid, user_id: Uuid) {
    if let Err(err) = process_floorplan_inner(&state, floorplan_id, user_id).await {
        tracing::error!(%floorplan_id, error = %err, "floorplan processing failed");
        let _ = mark_failed(&state, floorplan_id, &err.to_string()).await;
    }
}

async fn process_floorplan_inner(
    state: &AppState,
    floorplan_id: Uuid,
    user_id: Uuid,
) -> anyhow::Result<()> {
    update_job(state, floorplan_id, "processing", 8, "Loading GLB").await?;

    let source_path: String =
        sqlx::query_scalar("SELECT source_artifact_path FROM floorplans WHERE id = $1")
            .bind(floorplan_id)
            .fetch_one(&state.pool)
            .await?;
    let source_bytes = state.store.read(&source_path).await?;

    update_job(
        state,
        floorplan_id,
        "processing",
        24,
        "Analyzing 3D geometry",
    )
    .await?;
    let scan = gltf_import::scan_glb(&source_bytes)?;

    update_job(
        state,
        floorplan_id,
        "processing",
        46,
        "Detecting rooms and openings",
    )
    .await?;
    let document = geometry::build_floorplan(floorplan_id, scan)?;

    update_job(
        state,
        floorplan_id,
        "processing",
        64,
        "Calculating measurements",
    )
    .await?;
    let json = serde_json::to_vec_pretty(&document)?;
    let svg = render::render_svg(&document);

    update_job(
        state,
        floorplan_id,
        "processing",
        82,
        "Generating floorplan PDF",
    )
    .await?;
    let pdf = render::render_pdf(&document);

    let json_artifact = state
        .store
        .put(floorplan_id, "floorplan.json", &json)
        .await?;
    let svg_artifact = state
        .store
        .put(floorplan_id, "floorplan.svg", svg.as_bytes())
        .await?;
    let pdf_artifact = state.store.put(floorplan_id, "floorplan.pdf", &pdf).await?;
    let thumb_artifact = state
        .store
        .put(floorplan_id, "thumbnail.svg", svg.as_bytes())
        .await?;

    insert_artifact(
        state,
        floorplan_id,
        "json",
        "application/json",
        &json_artifact,
    )
    .await?;
    insert_artifact(state, floorplan_id, "svg", "image/svg+xml", &svg_artifact).await?;
    insert_artifact(state, floorplan_id, "pdf", "application/pdf", &pdf_artifact).await?;
    insert_artifact(
        state,
        floorplan_id,
        "thumbnail",
        "image/svg+xml",
        &thumb_artifact,
    )
    .await?;

    let month_start = auth::current_month_start();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE floorplans
        SET status = 'complete',
            floorplan_json_path = $2,
            svg_path = $3,
            pdf_path = $4,
            thumbnail_path = $5,
            confidence = $6,
            total_area_sqft = $7,
            width_ft = $8,
            depth_ft = $9,
            failure_reason = NULL,
            updated_at = now()
        WHERE id = $1 AND user_id = $10
        "#,
    )
    .bind(floorplan_id)
    .bind(json_artifact.relative_path)
    .bind(svg_artifact.relative_path)
    .bind(pdf_artifact.relative_path)
    .bind(thumb_artifact.relative_path)
    .bind(document.confidence)
    .bind(document.total_area_sqft)
    .bind(document.width_ft)
    .bind(document.depth_ft)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO monthly_save_events (id, user_id, floorplan_id, month_start)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (floorplan_id) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(floorplan_id)
    .bind(month_start)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE processing_jobs
        SET status = 'complete',
            progress = 100,
            step = 'Floorplan ready',
            error = NULL,
            updated_at = now()
        WHERE floorplan_id = $1
        "#,
    )
    .bind(floorplan_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

async fn update_job(
    state: &AppState,
    floorplan_id: Uuid,
    status: &str,
    progress: i32,
    step: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE processing_jobs
        SET status = $2, progress = $3, step = $4, error = NULL, updated_at = now()
        WHERE floorplan_id = $1
        "#,
    )
    .bind(floorplan_id)
    .bind(status)
    .bind(progress)
    .bind(step)
    .execute(&state.pool)
    .await?;

    Ok(())
}

async fn mark_failed(state: &AppState, floorplan_id: Uuid, message: &str) -> anyhow::Result<()> {
    let message = message.chars().take(800).collect::<String>();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        r#"
        UPDATE floorplans
        SET status = 'failed', failure_reason = $2, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(floorplan_id)
    .bind(&message)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE processing_jobs
        SET status = 'failed', progress = 100, step = 'Processing failed', error = $2, updated_at = now()
        WHERE floorplan_id = $1
        "#,
    )
    .bind(floorplan_id)
    .bind(&message)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

async fn insert_artifact(
    state: &AppState,
    floorplan_id: Uuid,
    kind: &str,
    content_type: &str,
    artifact: &storage::StoredArtifact,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO artifact_objects (id, floorplan_id, kind, path, content_type, size_bytes, sha256)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(floorplan_id)
    .bind(kind)
    .bind(&artifact.relative_path)
    .bind(content_type)
    .bind(artifact.size_bytes)
    .bind(&artifact.sha256)
    .execute(&state.pool)
    .await?;
    Ok(())
}
