mod auth;
mod config;
mod error;
mod models;
mod processing;
mod routes;
mod storage;

use std::sync::Arc;

use axum::Router;
use config::AppConfig;
use sqlx::postgres::PgPoolOptions;
use storage::ArtifactStore;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pool: sqlx::PgPool,
    pub store: ArtifactStore,
    pub http: reqwest::Client,
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
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let store = ArtifactStore::new(config.artifact_dir.clone()).await?;
    let state = AppState {
        config: config.clone(),
        pool,
        store,
        http: reqwest::Client::new(),
    };

    let cors = config.cors_layer()?;
    let app: Router = routes::router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("backend listening on http://{}", config.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
