use crate::parser::ExpenseParser;
use crate::repos::Repository;

pub struct AppState {
    pub repo: Repository,
    pub parser: tokio::sync::Mutex<ExpenseParser>,
    pub exchange_rate: f64,
}