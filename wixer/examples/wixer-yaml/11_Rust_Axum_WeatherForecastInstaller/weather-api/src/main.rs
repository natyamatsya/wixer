//! Weather Forecast API - Rust/Axum equivalent of ASP.NET Core WeatherForecast template
//!
//! A minimal REST API that returns random weather forecasts.
//! Demonstrates a single-binary deployment with no runtime dependencies.

use axum::{routing::get, Json, Router};
use chrono::{Duration, Utc};
use rand::Rng;
use serde::Serialize;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Weather forecast data matching the ASP.NET Core template
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeatherForecast {
    date: String,
    temperature_c: i32,
    temperature_f: i32,
    summary: &'static str,
}

const SUMMARIES: &[&str] = &[
    "Freezing",
    "Bracing",
    "Chilly",
    "Cool",
    "Mild",
    "Warm",
    "Balmy",
    "Hot",
    "Sweltering",
    "Scorching",
];

/// GET /weatherforecast - Returns 5 days of random weather forecasts
async fn get_weather_forecast() -> Json<Vec<WeatherForecast>> {
    let mut rng = rand::thread_rng();
    let today = Utc::now();

    let forecasts: Vec<WeatherForecast> = (1..=5)
        .map(|i| {
            let temp_c = rng.gen_range(-20..=55);
            let date = today + Duration::days(i);
            WeatherForecast {
                date: date.format("%Y-%m-%d").to_string(),
                temperature_c: temp_c,
                temperature_f: 32 + (temp_c as f64 * 1.8) as i32,
                summary: SUMMARIES[rng.gen_range(0..SUMMARIES.len())],
            }
        })
        .collect();

    Json(forecasts)
}

/// GET / - Health check / welcome endpoint
async fn root() -> &'static str {
    "Weather Forecast API v1.0.0 - Visit /weatherforecast for data"
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "weather_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Build router
    let app = Router::new()
        .route("/", get(root))
        .route("/weatherforecast", get(get_weather_forecast))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    // Bind to port (default 5000, or from WEATHER_API_PORT env var)
    let port: u16 = std::env::var("WEATHER_API_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Weather API listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
