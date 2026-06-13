use std::{env, net::SocketAddr, path::PathBuf};

use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub artifact_dir: PathBuf,
    pub public_base_url: String,
    pub frontend_origin: String,
    pub allowed_origins: Vec<String>,
    pub auth_secret: String,
    pub auth_cookie_name: String,
    pub auth_session_days: i64,
    pub google_client_id: Option<String>,
    pub max_upload_mb: usize,
    pub monthly_free_saves: i64,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env_or("BIND_ADDR", "0.0.0.0:8080").parse()?;
        let frontend_origin = env_or("FRONTEND_ORIGIN", "http://localhost:5173");
        let allowed_origins = env::var("AUTH_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| frontend_origin.clone())
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        Ok(Self {
            bind_addr,
            database_url: env_or(
                "DATABASE_URL",
                "postgres://floorplan:floorplan@localhost:5432/floorplan",
            ),
            artifact_dir: PathBuf::from(env_or("ARTIFACT_DIR", "./data/artifacts")),
            public_base_url: env_or("PUBLIC_BASE_URL", "http://localhost:8080"),
            frontend_origin,
            allowed_origins,
            auth_secret: env_or("AUTH_SECRET", "dev-only-change-me"),
            auth_cookie_name: env_or("AUTH_COOKIE_NAME", "glb_floorplan_session"),
            auth_session_days: env_or("AUTH_SESSION_DAYS", "30").parse()?,
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok().filter(|v| !v.is_empty()),
            max_upload_mb: env_or("MAX_UPLOAD_MB", "250").parse()?,
            monthly_free_saves: env_or("MONTHLY_FREE_SAVES", "5").parse()?,
        })
    }

    pub fn cors_layer(&self) -> anyhow::Result<CorsLayer> {
        let origins = self
            .allowed_origins
            .iter()
            .map(|origin| origin.parse::<HeaderValue>())
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_credentials(true)
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                axum::http::header::ACCEPT,
            ])
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::DELETE,
                Method::OPTIONS,
                Method::HEAD,
            ]))
    }
}

fn env_or(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_owned())
}
