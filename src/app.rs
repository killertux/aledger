use aws_sdk_dynamodb::Client;
use axum::Router;
use axum::routing::{delete, get, post};
use rand::prelude::SmallRng;

use crate::{controller, root};

#[derive(Clone, Debug)]
pub struct AppState {
    pub dynamo_client: Client,
    pub random_number_generator: SmallRng,
}

pub fn build_app(client: Client, rng: SmallRng) -> Router {
    Router::new()
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
            random_number_generator: rng,
        })
}
