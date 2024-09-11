use aws_sdk_dynamodb::Client;
use axum::routing::{delete, get, post};
use axum::Router;
use rand::prelude::SmallRng;

use crate::controller;

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

async fn root() -> &'static str {
    "Hello, World!"
}

#[cfg(test)]
pub mod test {
    use aws_sdk_dynamodb::Client;
    use lazy_static::lazy_static;
    use rand::SeedableRng;
    use rand::{rngs::SmallRng, Rng};
    use tokio::sync::Mutex;

    use crate::{
        domain::gateway::LedgerEntryRepository,
        gateway::ledger_entry_repository::DynamoDbLedgerEntryRepository,
    };

    lazy_static! {
        static ref CLIENT: Mutex<Option<Client>> = Mutex::new(None);
        static ref RNG: Mutex<Option<SmallRng>> = Mutex::new(None);
    }

    pub async fn set_up_dynamo_db_for_test() -> Client {
        let mut client = CLIENT.lock().await;
        match client.as_ref() {
            Some(client) => client.clone(),
            None => {
                dotenv::dotenv().expect("Error loading env for test");
                let new_client = crate::dynamo_db_client().await;
                *client = Some(new_client.clone());
                new_client
            }
        }
    }

    pub async fn get_rng() -> impl Rng {
        let mut rng = RNG.lock().await;
        match rng.as_ref() {
            Some(rng) => rng.clone(),
            None => {
                let new_rng = SmallRng::from_entropy();
                *rng = Some(new_rng.clone());
                new_rng
            }
        }
    }

    pub async fn get_repository() -> impl LedgerEntryRepository {
        DynamoDbLedgerEntryRepository::from(set_up_dynamo_db_for_test().await)
    }
}
