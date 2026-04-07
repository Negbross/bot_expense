use std::env;
use chrono::Utc;
use sea_orm::{ActiveValue::Set, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter};
use migration::{Migrator, MigratorTrait};
use uuid::Uuid;

use crate::entity::users;

pub async fn init_db() -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    // Connect mapping to sea_orm
    let db: DatabaseConnection = Database::connect(&database_url).await?;

    // Run migrations
    Migrator::up(&db, None).await?;

    // Seed admin user
    seed_admin_user(&db).await?;

    Ok(db)
}

pub async fn seed_admin_user(db: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
    let admin_id = env::var("ADMIN_ID").expect("ADMIN_ID must be set");
    if users::Entity::find()
    .filter(users::Column::TelegramId.eq(admin_id.parse::<i64>().expect("empty admin id env")))
    .one(db).await?.is_some() {
        return Ok(());
    }
    let admin = users::ActiveModel {
        id: Set(Uuid::new_v4()),
        telegram_id: Set(admin_id.parse::<i64>().expect("empty admin id env")),
        is_admin: Set(true),
        is_whitelisted: Set(true),
        created_at: Set(Some(Utc::now().fixed_offset())),
        ..Default::default()
    };
    users::Entity::insert(admin).exec(db).await?;
    Ok(())
}
