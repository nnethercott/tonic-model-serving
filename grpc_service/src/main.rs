use grpc_service::{config::get_config, server::run_server};
use tracing::{debug, error, info};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    let config = get_config().expect("failed to build config");
    let log_level = config.log_level.clone().as_str();

    tracing_subscriber::registry()
        .with(JsonStorageLayer)
        .with(
            BunyanFormattingLayer::new("grpc-service".into(), std::io::stdout)
                .skip_fields(vec!["file", "line", "target"].into_iter())
                .unwrap(),
        )
        .with(EnvFilter::try_from_default_env().unwrap_or(log_level.into()))
        .init();

    let handle = tokio::spawn(async move {
        info!("starting server...");
        debug!(config=?config.server.addr());

        if let Err(e) = run_server(config).await {
            error!(error=%e, "server error");
        };
    });

    tokio::select! {
        _ = handle => {},
    }
}
