use std::{convert::Infallible, time::Duration};

use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState, auth,
    error::{ApiError, ApiResult},
    models::{
        AuthResponse, FloorplanDetail, FloorplanRow, FloorplanSummary, GoogleAuthRequest,
        JobSnapshot, MeResponse, ProcessingJobRow, PublicUser, UploadResponse,
    },
    processing,
    storage::sha256_hex,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/auth/google", post(google_auth))
        .route("/api/me", get(me))
        .route(
            "/api/floorplans",
            get(list_floorplans).post(upload_floorplan),
        )
        .route("/api/floorplans/{id}", get(get_floorplan))
        .route("/api/floorplans/{id}/events", get(floorplan_events))
        .route("/api/floorplans/{id}.svg", get(get_svg))
        .route("/api/floorplans/{id}.pdf", get(get_pdf))
        .with_state(state)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn google_auth(
    State(state): State<AppState>,
    Json(body): Json<GoogleAuthRequest>,
) -> ApiResult<Response> {
    let google = auth::verify_google_id_token(&state, &body.id_token).await?;
    let user = auth::upsert_user(&state, google).await?;
    let quota = auth::quota_for_user(&state, user.id).await?;
    let (token, expires_at) = auth::create_session(&state, user.id).await?;
    let response = AuthResponse {
        token: token.clone(),
        user: PublicUser::from(user),
        quota,
    };

    let mut res = Json(response).into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        auth::set_cookie_header(auth::auth_cookie(&state.config, &token, expires_at)),
    );
    Ok(res)
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<MeResponse>> {
    let user = auth::require_user_from_headers(&state, &headers).await?;
    let quota = auth::quota_for_user(&state, user.id).await?;
    Ok(Json(MeResponse {
        user: user.into(),
        quota,
    }))
}

async fn upload_floorplan(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    let user = auth::require_user_from_headers(&state, &headers).await?;
    let quota = auth::quota_for_user(&state, user.id).await?;
    if quota.remaining <= 0 {
        return Err(ApiError::QuotaExceeded);
    }

    let mut upload: Option<(String, Vec<u8>)> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::BadRequest(format!("invalid multipart upload: {err}")))?
    {
        let name = field.name().unwrap_or("").to_owned();
        if name == "file" || name == "glb" {
            let filename = field.file_name().unwrap_or("upload.glb").to_owned();
            let bytes = field
                .bytes()
                .await
                .map_err(|err| ApiError::BadRequest(format!("could not read upload: {err}")))?
                .to_vec();
            upload = Some((filename, bytes));
            break;
        }
    }

    let Some((filename, bytes)) = upload else {
        return Err(ApiError::BadRequest("missing GLB file field".to_owned()));
    };

    if !filename.to_ascii_lowercase().ends_with(".glb")
        && !filename.to_ascii_lowercase().ends_with(".gltf")
    {
        return Err(ApiError::BadRequest(
            "file must be .glb or .gltf".to_owned(),
        ));
    }
    if bytes.is_empty() {
        return Err(ApiError::BadRequest("uploaded file is empty".to_owned()));
    }
    let max_bytes = state.config.max_upload_mb * 1024 * 1024;
    if bytes.len() > max_bytes {
        return Err(ApiError::BadRequest(format!(
            "file exceeds {}MB upload limit",
            state.config.max_upload_mb
        )));
    }

    let floorplan_id = Uuid::new_v4();
    let source = state.store.put(floorplan_id, "source.glb", &bytes).await?;
    let source_hash = sha256_hex(&bytes);
    let title = filename
        .trim_end_matches(".glb")
        .trim_end_matches(".gltf")
        .replace(['_', '-'], " ");

    let mut tx = state.pool.begin().await?;
    let row = sqlx::query_as::<_, FloorplanRow>(
        r#"
        INSERT INTO floorplans (
          id, user_id, title, status, source_filename, source_size_bytes,
          source_sha256, source_artifact_path
        )
        VALUES ($1, $2, $3, 'processing', $4, $5, $6, $7)
        RETURNING id, user_id, title, status, source_filename, source_size_bytes,
          source_sha256, source_artifact_path, floorplan_json_path, svg_path, pdf_path,
          thumbnail_path, confidence, total_area_sqft, width_ft, depth_ft, failure_reason,
          created_at, updated_at
        "#,
    )
    .bind(floorplan_id)
    .bind(user.id)
    .bind(if title.trim().is_empty() {
        "New Floorplan".to_owned()
    } else {
        title
    })
    .bind(&filename)
    .bind(bytes.len() as i64)
    .bind(source_hash)
    .bind(source.relative_path)
    .fetch_one(&mut *tx)
    .await?;

    let job = sqlx::query_as::<_, ProcessingJobRow>(
        r#"
        INSERT INTO processing_jobs (id, floorplan_id, status, progress, step)
        VALUES ($1, $2, 'queued', 0, 'Queued')
        RETURNING id, floorplan_id, status, progress, step, error, created_at, updated_at
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(floorplan_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    tokio::spawn(processing::process_floorplan(
        state.clone(),
        floorplan_id,
        user.id,
    ));

    Ok(Json(UploadResponse {
        floorplan: summary_from_row(row),
        job: snapshot_from_job(job),
    }))
}

async fn list_floorplans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<FloorplanSummary>>> {
    let user = auth::require_user_from_headers(&state, &headers).await?;
    let rows = sqlx::query_as::<_, FloorplanRow>(
        r#"
        SELECT id, user_id, title, status, source_filename, source_size_bytes,
          source_sha256, source_artifact_path, floorplan_json_path, svg_path, pdf_path,
          thumbnail_path, confidence, total_area_sqft, width_ft, depth_ft, failure_reason,
          created_at, updated_at
        FROM floorplans
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.into_iter().map(summary_from_row).collect()))
}

async fn get_floorplan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<FloorplanDetail>> {
    let user = auth::require_user_from_headers(&state, &headers).await?;
    let row = load_floorplan(&state, user.id, id).await?;
    let job = load_job(&state, id).await.ok();
    Ok(Json(FloorplanDetail {
        floorplan: summary_from_row(row),
        job,
    }))
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

async fn floorplan_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<TokenQuery>,
) -> ApiResult<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>> {
    let user = auth_from_headers_or_query(&state, &headers, query.token.as_deref()).await?;
    let _ = load_floorplan(&state, user.id, id).await?;

    let stream = async_stream::stream! {
        loop {
            match load_job(&state, id).await {
                Ok(snapshot) => {
                    let terminal = snapshot.status == "complete" || snapshot.status == "failed";
                    let event = Event::default()
                        .event("progress")
                        .json_data(&snapshot)
                        .unwrap_or_else(|_| Event::default().data("{\"status\":\"error\"}"));
                    yield Ok(event);
                    if terminal {
                        break;
                    }
                }
                Err(_) => {
                    yield Ok(Event::default().event("error").data("job not found"));
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(900)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn get_svg(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<TokenQuery>,
) -> ApiResult<Response> {
    let user = auth_from_headers_or_query(&state, &headers, query.token.as_deref()).await?;
    let row = load_floorplan(&state, user.id, id).await?;
    let path = row.svg_path.ok_or(ApiError::NotFound)?;
    artifact_response(&state, &path, "image/svg+xml", None).await
}

async fn get_pdf(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<TokenQuery>,
) -> ApiResult<Response> {
    let user = auth_from_headers_or_query(&state, &headers, query.token.as_deref()).await?;
    let row = load_floorplan(&state, user.id, id).await?;
    let path = row.pdf_path.ok_or(ApiError::NotFound)?;
    artifact_response(
        &state,
        &path,
        "application/pdf",
        Some(format!("{}.pdf", row.title.replace('/', "-"))),
    )
    .await
}

async fn artifact_response(
    state: &AppState,
    path: &str,
    content_type: &'static str,
    filename: Option<String>,
) -> ApiResult<Response> {
    let bytes = state.store.read(path).await?;
    let mut response = (StatusCode::OK, bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    if let Some(filename) = filename {
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename.replace('"', ""))
                .parse()
                .unwrap(),
        );
    }
    Ok(response)
}

async fn auth_from_headers_or_query(
    state: &AppState,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> ApiResult<crate::models::User> {
    if let Some(token) = query_token.filter(|t| !t.is_empty()) {
        return auth::require_user_from_token(state, token).await;
    }
    auth::require_user_from_headers(state, headers).await
}

async fn load_floorplan(state: &AppState, user_id: Uuid, id: Uuid) -> ApiResult<FloorplanRow> {
    sqlx::query_as::<_, FloorplanRow>(
        r#"
        SELECT id, user_id, title, status, source_filename, source_size_bytes,
          source_sha256, source_artifact_path, floorplan_json_path, svg_path, pdf_path,
          thumbnail_path, confidence, total_area_sqft, width_ft, depth_ft, failure_reason,
          created_at, updated_at
        FROM floorplans
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)
}

async fn load_job(state: &AppState, floorplan_id: Uuid) -> ApiResult<JobSnapshot> {
    let row = sqlx::query_as::<_, ProcessingJobRow>(
        r#"
        SELECT id, floorplan_id, status, progress, step, error, created_at, updated_at
        FROM processing_jobs
        WHERE floorplan_id = $1
        "#,
    )
    .bind(floorplan_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    Ok(snapshot_from_job(row))
}

fn snapshot_from_job(row: ProcessingJobRow) -> JobSnapshot {
    JobSnapshot {
        floorplan_id: row.floorplan_id,
        status: row.status,
        progress: row.progress,
        step: row.step,
        error: row.error,
    }
}

fn summary_from_row(row: FloorplanRow) -> FloorplanSummary {
    let svg_url = row
        .svg_path
        .as_ref()
        .map(|_| format!("/api/floorplans/{}.svg", row.id));
    let pdf_url = row
        .pdf_path
        .as_ref()
        .map(|_| format!("/api/floorplans/{}.pdf", row.id));

    FloorplanSummary {
        id: row.id,
        title: row.title,
        status: row.status,
        source_filename: row.source_filename,
        source_size_bytes: row.source_size_bytes,
        confidence: row.confidence,
        total_area_sqft: row.total_area_sqft,
        width_ft: row.width_ft,
        depth_ft: row.depth_ft,
        failure_reason: row.failure_reason,
        created_at: row.created_at,
        svg_url,
        pdf_url,
    }
}
