use std::{
    convert::Infallible,
    io::{Cursor, Read},
    net::SocketAddr,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use chrono::{Datelike, Days, NaiveDate, TimeZone, Utc};
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    AppState, DailyUsage,
    error::{ApiError, ApiResult},
    models::{
        FloorplanDetail, FloorplanRecord, FloorplanSummary, JobSnapshot, Quota, UploadResponse,
    },
    processing,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/quota", get(get_quota))
        .route("/api/floorplans", post(upload_floorplan))
        .route("/api/floorplans/{id}", get(get_floorplan))
        .route("/api/floorplans/{id}/events", get(floorplan_events))
        .route("/api/floorplans/{id}/svg", get(get_svg))
        .route("/api/floorplans/{id}/pdf", get(get_pdf))
        .with_state(state)
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn get_quota(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Json<Quota> {
    Json(quota_for_request(&state, &headers, addr).await)
}

async fn upload_floorplan(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
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

    let Some((upload_filename, upload_bytes)) = upload else {
        return Err(ApiError::BadRequest("missing model file field".to_owned()));
    };

    if upload_bytes.is_empty() {
        return Err(ApiError::BadRequest("uploaded file is empty".to_owned()));
    }
    let max_bytes = state.config.max_upload_mb * 1024 * 1024;
    if upload_bytes.len() > max_bytes {
        return Err(ApiError::BadRequest(format!(
            "file exceeds {}MB upload limit",
            state.config.max_upload_mb
        )));
    }
    let UploadedModel { filename, bytes } =
        normalize_uploaded_model(upload_filename, upload_bytes, max_bytes)?;

    let quota = consume_quota(&state, &headers, addr).await?;
    let floorplan_id = Uuid::new_v4();
    let title = title_from_model_filename(&filename);
    let created_at = Utc::now();
    let floorplan = FloorplanSummary {
        id: floorplan_id,
        title: if title.trim().is_empty() {
            "New Floorplan".to_owned()
        } else {
            title
        },
        status: "queued".to_owned(),
        source_filename: filename,
        source_size_bytes: bytes.len() as i64,
        confidence: 0.0,
        total_area_sqft: None,
        width_ft: None,
        depth_ft: None,
        failure_reason: None,
        created_at,
        svg_url: None,
        pdf_url: None,
    };
    let job = JobSnapshot {
        floorplan_id,
        status: "queued".to_owned(),
        progress: 0,
        step: "Queued".to_owned(),
        error: None,
    };

    state.jobs.write().await.insert(
        floorplan_id,
        FloorplanRecord {
            summary: floorplan.clone(),
            job: job.clone(),
            svg: None,
            pdf: None,
        },
    );

    tokio::spawn(processing::process_floorplan(
        state.clone(),
        floorplan_id,
        bytes,
    ));

    Ok(Json(UploadResponse {
        floorplan,
        job,
        quota,
    }))
}

#[derive(Debug)]
struct UploadedModel {
    filename: String,
    bytes: Vec<u8>,
}

fn normalize_uploaded_model(
    filename: String,
    bytes: Vec<u8>,
    max_bytes: usize,
) -> ApiResult<UploadedModel> {
    let lower = filename.to_ascii_lowercase();
    if lower.ends_with(".glb") || lower.ends_with(".gltf") {
        return Ok(UploadedModel { filename, bytes });
    }
    if lower.ends_with(".zip") {
        return extract_model_from_zip(&filename, bytes, max_bytes);
    }

    Err(ApiError::BadRequest(
        "file must be .glb, .gltf, or a .zip containing one".to_owned(),
    ))
}

fn extract_model_from_zip(
    archive_filename: &str,
    bytes: Vec<u8>,
    max_bytes: usize,
) -> ApiResult<UploadedModel> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|err| ApiError::BadRequest(format!("could not read zip archive: {err}")))?;
    let mut gltf_candidate: Option<UploadedModel> = None;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| ApiError::BadRequest(format!("could not read zip entry: {err}")))?;
        if entry.is_dir() {
            continue;
        }

        let entry_name = entry.name().to_owned();
        let lower = entry_name.to_ascii_lowercase();
        if lower.starts_with("__macosx/") || lower.contains("/__macosx/") {
            continue;
        }
        if !lower.ends_with(".glb") && !lower.ends_with(".gltf") {
            continue;
        }

        let mut extracted = Vec::new();
        entry
            .by_ref()
            .take((max_bytes + 1) as u64)
            .read_to_end(&mut extracted)
            .map_err(|err| {
                ApiError::BadRequest(format!("could not extract model from zip: {err}"))
            })?;
        if extracted.len() > max_bytes {
            return Err(ApiError::BadRequest(format!(
                "model inside zip exceeds {}MB upload limit",
                max_bytes / 1024 / 1024
            )));
        }
        if extracted.is_empty() {
            return Err(ApiError::BadRequest(format!(
                "model inside zip is empty: {entry_name}"
            )));
        }

        let filename = format!("{archive_filename}: {entry_name}");
        let model = UploadedModel {
            filename,
            bytes: extracted,
        };
        if lower.ends_with(".glb") {
            return Ok(model);
        }
        gltf_candidate = Some(model);
    }

    gltf_candidate.ok_or_else(|| {
        ApiError::BadRequest("zip archive did not contain a .glb or .gltf file".to_owned())
    })
}

fn title_from_model_filename(filename: &str) -> String {
    let model_name = filename
        .rsplit_once(": ")
        .map(|(_, inner)| inner)
        .unwrap_or(filename)
        .rsplit('/')
        .next()
        .unwrap_or(filename);
    let lower = model_name.to_ascii_lowercase();
    let stem = if lower.ends_with(".glb") || lower.ends_with(".zip") {
        &model_name[..model_name.len().saturating_sub(4)]
    } else if lower.ends_with(".gltf") {
        &model_name[..model_name.len().saturating_sub(5)]
    } else {
        model_name
    };
    stem.replace(['_', '-'], " ")
}

async fn floorplan_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>> {
    if load_record(&state, id).await.is_none() {
        return Err(ApiError::NotFound);
    }

    let stream = async_stream::stream! {
        loop {
            match load_record(&state, id).await {
                Some(record) => {
                    let snapshot = record.job;
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
                None => {
                    yield Ok(Event::default().event("error").data("job not found"));
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(900)).await;
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn get_floorplan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<FloorplanDetail>> {
    let record = load_record(&state, id).await.ok_or(ApiError::NotFound)?;
    Ok(Json(FloorplanDetail {
        floorplan: record.summary,
        job: Some(record.job),
    }))
}

async fn get_svg(State(state): State<AppState>, Path(id): Path<Uuid>) -> ApiResult<Response> {
    let record = load_record(&state, id).await.ok_or(ApiError::NotFound)?;
    let svg = record.svg.ok_or(ApiError::NotFound)?;
    let mut response = (StatusCode::OK, svg).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "image/svg+xml".parse().unwrap());
    Ok(response)
}

async fn get_pdf(State(state): State<AppState>, Path(id): Path<Uuid>) -> ApiResult<Response> {
    let record = load_record(&state, id).await.ok_or(ApiError::NotFound)?;
    let pdf = record.pdf.ok_or(ApiError::NotFound)?;
    let mut response = (StatusCode::OK, pdf).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!(
            "attachment; filename=\"{}.pdf\"",
            record.summary.title.replace(['/', '"'], "-")
        )
        .parse()
        .unwrap(),
    );
    Ok(response)
}

async fn load_record(state: &AppState, id: Uuid) -> Option<FloorplanRecord> {
    state.jobs.read().await.get(&id).cloned()
}

async fn quota_for_request(state: &AppState, headers: &HeaderMap, addr: SocketAddr) -> Quota {
    let ip = client_ip(headers, addr);
    let today = Utc::now().date_naive();
    let usage = state.usage.lock().await;
    let used = usage
        .get(&ip)
        .filter(|entry| entry.day == today)
        .map(|entry| entry.used)
        .unwrap_or(0);
    quota_from_used(state, today, used)
}

async fn consume_quota(
    state: &AppState,
    headers: &HeaderMap,
    addr: SocketAddr,
) -> ApiResult<Quota> {
    let ip = client_ip(headers, addr);
    let today = Utc::now().date_naive();
    let mut usage = state.usage.lock().await;
    let entry = usage.entry(ip).or_insert(DailyUsage {
        day: today,
        used: 0,
    });
    if entry.day != today {
        entry.day = today;
        entry.used = 0;
    }
    if entry.used >= state.config.daily_ip_converts {
        return Err(ApiError::QuotaExceeded);
    }
    entry.used += 1;
    Ok(quota_from_used(state, today, entry.used))
}

fn quota_from_used(state: &AppState, day: NaiveDate, used: i64) -> Quota {
    let daily_limit = state.config.daily_ip_converts;
    Quota {
        daily_limit,
        used,
        remaining: (daily_limit - used).max(0),
        day,
        reset_at: next_day_start_utc(day),
    }
}

fn next_day_start_utc(day: NaiveDate) -> chrono::DateTime<Utc> {
    let next = day.checked_add_days(Days::new(1)).expect("valid next day");
    Utc.with_ymd_and_hms(next.year(), next.month(), next.day(), 0, 0, 0)
        .single()
        .expect("valid date")
}

fn client_ip(headers: &HeaderMap, addr: SocketAddr) -> String {
    for header_name in ["x-forwarded-for", "x-real-ip", "cf-connecting-ip"] {
        if let Some(value) = headers.get(header_name).and_then(|v| v.to_str().ok()) {
            if let Some(ip) = value
                .split(',')
                .next()
                .map(str::trim)
                .filter(|v| !v.is_empty())
            {
                return ip.to_owned();
            }
        }
    }
    addr.ip().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};

    #[test]
    fn extracts_nested_glb_from_zip() {
        let zip_bytes = zip_with_files(&[
            ("assets/readme.txt", b"ignore me".as_slice()),
            ("models/apartment.glb", b"glb bytes".as_slice()),
        ]);

        let model =
            normalize_uploaded_model("scan.zip".to_owned(), zip_bytes, 1024).expect("valid model");

        assert_eq!(model.filename, "scan.zip: models/apartment.glb");
        assert_eq!(model.bytes, b"glb bytes");
    }

    #[test]
    fn rejects_zip_without_model() {
        let zip_bytes = zip_with_files(&[("assets/readme.txt", b"ignore me".as_slice())]);

        let err = normalize_uploaded_model("scan.zip".to_owned(), zip_bytes, 1024)
            .expect_err("zip should not contain a model");

        assert!(err.to_string().contains("did not contain a .glb or .gltf"));
    }

    fn zip_with_files(files: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        for (name, bytes) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("start zip file");
            writer.write_all(bytes).expect("write zip file");
        }
        writer.finish().expect("finish zip").into_inner()
    }
}
