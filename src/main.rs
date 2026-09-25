use std::net::SocketAddr;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ezrouter::system_log::SystemLogLayer;
use ezrouter::{app_router, AppState, Config};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ezrouter=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(SystemLogLayer)
        .init();

    info!("Initializing ezRouter service...");

    let config = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("Configuration error: {err}");
            std::process::exit(1);
        }
    };

    if let Err(e) = std::fs::create_dir_all(&config.data_dir) {
        error!(
            "Failed to initialize staging data directory {}: {e}",
            config.data_dir.display()
        );
        std::process::exit(1);
    }

    let addr_str = format!("{}:{}", config.host, config.port);
    let addr: SocketAddr = match addr_str.parse() {
        Ok(a) => a,
        Err(e) => {
            error!("Invalid bind address '{addr_str}': {e}");
            std::process::exit(1);
        }
    };

    let data_dir_display = config.data_dir.display().to_string();
    let state = AppState::new(config);
    let app = app_router(state);

    info!(
        "Listening on http://{} (staging data_dir: {})",
        addr, data_dir_display
    );

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind on {addr}: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        error!("Server error: {e}");
        std::process::exit(1);
    }

    info!("Graceful shutdown complete; in-flight requests drained");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C / SIGINT signal, initiating graceful shutdown");
        },
        _ = terminate => {
            info!("Received SIGTERM signal, initiating graceful shutdown");
        },
    }
}
