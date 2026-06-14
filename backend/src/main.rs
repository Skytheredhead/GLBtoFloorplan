mod config;
mod error;
mod models;
mod processing;
mod routes;

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::Router;
use chrono::NaiveDate;
use config::AppConfig;
use models::FloorplanRecord;
use tokio::sync::{Mutex, RwLock};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub jobs: Arc<RwLock<HashMap<Uuid, FloorplanRecord>>>,
    pub usage: Arc<Mutex<HashMap<String, DailyUsage>>>,
}

#[derive(Debug, Clone)]
pub struct DailyUsage {
    pub day: NaiveDate,
    pub used: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "glb_floorplan_backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(AppConfig::from_env()?);
    let state = AppState {
        config: config.clone(),
        jobs: Arc::new(RwLock::new(HashMap::new())),
        usage: Arc::new(Mutex::new(HashMap::new())),
    };

    let cors = config.cors_layer()?;
    let app: Router = routes::router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("backend listening on http://{}", config.bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
