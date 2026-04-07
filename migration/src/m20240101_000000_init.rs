use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::{big_integer, pk_uuid, string, timestamp_with_time_zone, timestamp_with_time_zone_null};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    TelegramId,
    IsAdmin,
    IsWhitelisted,
    CreatedAt,
    UpdatedAt
}

#[derive(DeriveIden)]
enum Banned {
    Table,
    Id,
    TelegramId,
    CreatedAt,
    UpdatedAt
}

#[derive(DeriveIden)]
enum Expenses {
    Table,
    Id,
    UserId,
    ItemName,
    CategoryGroup,
    Amount,
    Description,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key().default(Expr::cust("gen_random_uuid()")))
                    .col(ColumnDef::new(Users::TelegramId).big_integer().not_null().unique_key())
                    .col(ColumnDef::new(Users::IsAdmin).boolean().not_null().default(false))
                    .col(ColumnDef::new(Users::IsWhitelisted).boolean().not_null().default(false))
                    .col(ColumnDef::new(Users::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .col(timestamp_with_time_zone_null(Users::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Expenses::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Expenses::Id).uuid().not_null().primary_key().default(Expr::cust("gen_random_uuid()")))
                    .col(ColumnDef::new(Expenses::UserId).uuid().not_null())
                    .col(string(Expenses::ItemName).not_null())
                    .col(string(Expenses::CategoryGroup).not_null())
                    .col(ColumnDef::new(Expenses::Amount).double().not_null())
                    .col(ColumnDef::new(Expenses::Description).text().not_null())
                    .col(ColumnDef::new(Expenses::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-expense-user_id")
                            .from(Expenses::Table, Expenses::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager.create_table(
            Table::create()
                .table(Banned::Table)
                .if_not_exists()
                .col(pk_uuid(Banned::Id).uuid().not_null().default(Expr::cust("gen_random_uuid()")))
                .col(big_integer(Banned::TelegramId).big_integer().not_null())
                .col(timestamp_with_time_zone(Banned::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                .col(timestamp_with_time_zone_null(Banned::UpdatedAt).timestamp_with_time_zone())
                .to_owned()
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Expenses::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;

        Ok(())
    }
}
