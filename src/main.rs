use anyhow::Result;
use aws_sdk_dynamodb as dynamodb;
use axum::routing::{delete, get, post};
use axum::Router;
use dynamodb::Client;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::Level;

mod controller;
mod domain;
mod gateway;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let config = aws_config::load_from_env().await;
    let dynamodb_local_config = aws_sdk_dynamodb::config::Builder::from(&config)
        .endpoint_url("http://localhost:8000")
        .build();
    let client = aws_sdk_dynamodb::Client::from_conf(dynamodb_local_config);

    gateway::build_database(&client).await?;

    let app = Router::new()
        .route("/", get(root))
        .nest(
            "/api/v1",
            Router::new()
                .route("/balance", post(controller::push_entries::push_entries))
                .route(
                    "/balance",
                    delete(controller::delete_entries::delete_entries),
                )
                .route(
                    "/balance/:account_id",
                    get(controller::get_balance::get_balance),
                )
                .route(
                    "/balance/:account_id/entry",
                    get(controller::get_entries::get_entries),
                )
                .route(
                    "/balance/:account_id/entry/:entry_id",
                    get(controller::get_entry::get_entry),
                ),
        )
        .with_state(AppState {
            dynamo_client: client,
            random_number_generator: SmallRng::from_entropy(),
        })
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001").await?;
    tracing::info!("Listening to new connections");
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Clone, Debug)]
struct AppState {
    pub dynamo_client: Client,
    pub random_number_generator: SmallRng,
}

async fn root() -> &'static str {
    "Hello, World!"
}
