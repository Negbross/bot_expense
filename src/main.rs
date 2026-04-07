pub mod db;
pub mod entity;
pub mod repos;
pub mod parser;
pub mod handlers;
mod utils;

use dotenvy::dotenv;
use tracing::info;
use tokio::runtime::Builder;
use crate::db::init_db;
use crate::repos::Repository;
use crate::parser::ExpenseParser;
use crate::handlers::run_bot;


fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .with_max_level(tracing::Level::DEBUG)
    .init();
    
    // Inisialisasi runtime dengan stack size 8MB
    let rt = Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .expect("Thread error");

    // Jalankan mesin async
    rt.block_on(async {
        info!("Starting Expense Tracker bot...");

        let pool = init_db().await.expect("Failed to initialize database");
        info!("Database initialized");
        let repo = Repository::new(pool);
        info!("Repository initialized");

        let parser = ExpenseParser::new("assets/expense_model_quant.onnx", "assets/tokenizer.json")
            .unwrap_or_else(|e| {
                panic!("Model files not found or failed to load: {}", e);
            });

        run_bot(repo, parser).await;
    });
}
