use std::{env, net::SocketAddr};

use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub public_base_url: String,
    pub frontend_origin: String,
    pub allowed_origins: Vec<String>,
    pub max_upload_mb: usize,
    pub daily_ip_converts: i64,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env_or("BIND_ADDR", "0.0.0.0:8080").parse()?;
        let frontend_origin = env_or("FRONTEND_ORIGIN", "http://localhost:5173");
        let allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_else(|_| frontend_origin.clone())
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        Ok(Self {
            bind_addr,
            public_base_url: env_or("PUBLIC_BASE_URL", "http://localhost:8080"),
            frontend_origin,
            allowed_origins,
            max_upload_mb: env_or("MAX_UPLOAD_MB", "250").parse()?,
            daily_ip_converts: env_or("DAILY_IP_CONVERTS", "5").parse()?,
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
            .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::ACCEPT])
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
