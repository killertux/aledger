use anyhow::Result;
use aws_sdk_dynamodb as dynamodb;
use clap::Parser;
use dotenv::{dotenv, var};
use dynamodb::Client;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::Level;

use crate::app::build_app;

mod app;
mod controller;
mod domain;
mod gateway;
mod utils;

#[derive(Debug, Parser)]
#[command(version, about)]
enum Args {
    /// Start the aldeger server
    Serve(ServerArgs),
    /// Create the dynamodb table
    DbCreate,
    /// Delete and recreate the dynamodb table
    DbReset,
}

#[derive(Debug, Parser)]
struct ServerArgs {
    /// Port to listen for. If not set it will try to load from ENV
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv()?;
    tracing_setup()?;

    let client = dynamo_db_client().await;
    let args = Args::parse();
    match args {
        Args::Serve(serve_args) => {
            let rng = SmallRng::from_entropy();
            let app = build_app(client, rng)
                .layer(CompressionLayer::new())
                .layer(TraceLayer::new_for_http());

            let port = serve_args.port.unwrap_or(var("PORT")?.parse()?);
            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
            tracing::info!("Listening to new connections at port {}", port);
            axum::serve(listener, app).await?;
        }
        Args::DbCreate => {
            gateway::create_database(&client).await?;
        }
        Args::DbReset => {
            gateway::delete_database(&client).await?;
            gateway::create_database(&client).await?;
        }
    }
    Ok(())
}

async fn dynamo_db_client() -> Client {
    let config = aws_config::load_from_env().await;
    let mut builder = aws_sdk_dynamodb::config::Builder::from(&config);
    if let Ok(url) = var("LOCAL_DYNAMO_DB_URL") {
        builder = builder.endpoint_url(url);
    }
    let dynamodb_local_config = builder.build();

    aws_sdk_dynamodb::Client::from_conf(dynamodb_local_config)
}

fn tracing_setup() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

async fn root() -> &'static str {
    "Hello, World!"
}
