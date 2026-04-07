use sea_orm::*;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sea_orm::sea_query::Expr;
use crate::entity::{users, expenses, banned};

pub struct Repository {
    pub db: DatabaseConnection,
}

impl Repository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn ensure_user(&self, telegram_id: i64) -> Result<users::Model, DbErr> {
        // Find existing
        let existing_user = users::Entity::find()
            .filter(users::Column::TelegramId.eq(telegram_id))
            .one(&self.db)
            .await?;

        if let Some(u) = existing_user {
            return Ok(u);
        }

        // Create new user
        let new_user = users::ActiveModel {
            id: Set(Uuid::new_v4()),
            telegram_id: Set(telegram_id),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;

        Ok(new_user)
    }

    pub async fn set_whitelist(&self, telegram_id: i64, status: bool) -> Result<(), DbErr> {
        users::Entity::update_many()
            .col_expr(users::Column::IsWhitelisted, Expr::value(status))
            .filter(users::Column::TelegramId.eq(telegram_id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn add_expense(&self, user_id: Uuid, amount: f64, description: &str) -> Result<expenses::Model, DbErr> {
        let expense = expenses::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            amount: Set(amount),
            description: Set(description.to_string()),
            ..Default::default()
        }
        .insert(&self.db)
        .await?;
        
        Ok(expense)
    }

    pub async fn get_user_expenses_since(&self, user_id: Uuid, since: DateTime<Utc>) -> Result<f64, DbErr> {
        let result: Option<f64> = expenses::Entity::find()
            .filter(expenses::Column::UserId.eq(user_id))
            .filter(expenses::Column::CreatedAt.gte(since))
            .select_only()
            .column_as(expenses::Column::Amount.sum(), "total")
            // IntoTuple maps the selected alias into a straight value tuple
            .into_tuple()
            .one(&self.db)
            .await?;

        Ok(result.unwrap_or(0.0))
    }

    pub async fn get_banned_user(&self, telegram_id: i64) -> Result<Option<banned::Model>, DbErr> {
        let user = users::Entity::find()
            .filter(users::Column::TelegramId.eq(telegram_id))
            .one(&self.db)
            .await?;
        if let Some(u) = user {
            let banned_user = banned::Entity::find()
                .filter(banned::Column::TelegramId.eq(u.telegram_id))
                .one(&self.db)
                .await?;

            Ok(banned_user)
        } else { Ok(None) }
    }

}
