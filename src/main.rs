extern crate core;

mod database;
mod indexer;
mod models;
mod ordinals;
mod schema;
mod util;

use crate::database::Database;
use crate::indexer::Indexer;
use crate::ordinals::Ordinals;
use dotenv::dotenv;
use std::env;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();

    ctrlc::set_handler(move || {
        if SHUTTING_DOWN.fetch_or(true, Ordering::Relaxed) {
            exit(1);
        }

        SHUTTING_DOWN.store(true, Ordering::SeqCst);
        println!("Ctrl+C received! Shutting down gracefully. Press <CTRL-C> again to shutdown immediately...");
    })
    .expect("Error setting Ctrl+C handler");

    let ordinals_url =
        env::var("ORDINALS_BASE_URL").expect("ORDINALS_BASE_URL must be set in .env file");
    let meta_protocol = env::var("META_PROTOCOL").expect("META_PROTOCOL must be set in .env file");
    let blocks_behind = env::var("BLOCKS_BEHIND")
        .unwrap_or_default()
        .parse::<i32>()
        .unwrap_or(0);

    let database = Database::new().unwrap();
    let ordinals: Ordinals = Ordinals::new(ordinals_url);

    let mut indexer = Indexer {
        ordinals,
        database,
        meta_protocol,
    };

    loop {
        let last_height = indexer
            .database
            .get_status("last_height".to_string())
            .expect("Database has not been correctly initialised. 'last_height' missing.")
            .value
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or_default();

        if let Ok(current_height) = indexer.ordinals.get_block_height().await {
            if last_height < current_height - blocks_behind {
                indexer.get_blocks(current_height - blocks_behind).await;
            }
        }

        if SHUTTING_DOWN.load(Ordering::Relaxed) {
            log::info!("Shutting down indexer.");
            break;
        }

        sleep(Duration::from_secs(5));
    }
}
