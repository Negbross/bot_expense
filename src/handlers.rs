use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use std::sync::Arc;
use chrono::{Datelike, TimeZone, Utc};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tracing::error;

use crate::repos::Repository;
use crate::parser::ExpenseParser;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "Start the bot and request access.")]
    Start,
    #[command(description = "Admin only: Whitelist a user by telegram ID.")]
    Whitelist(i64),
    #[command(description = "Admin only: Blacklist a user by telegram ID.")]
    Blacklist(i64),
    #[command(description = "Report expenses (e.g. /report daily, /report weekly, /report monthly).")]
    Report(String),
}

pub struct AppState {
    pub repo: Repository,
    pub parser: tokio::sync::Mutex<ExpenseParser>,
}

pub async fn run_bot(repo: Repository, parser: ExpenseParser) {
    let bot = Bot::from_env();
    let state = Arc::new(AppState { repo, parser: tokio::sync::Mutex::new(parser) });

    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(command_handler)
        )
        .branch(
            dptree::entry()
                .endpoint(message_handler)
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![state])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn command_handler(
    bot: Bot,
    msg: Message,
    cmd: Command,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    let telegram_id = msg.chat.id.0;
    
    // Ensure user exists in our DB
    let user = match state.repo.ensure_user(telegram_id).await {
        Ok(u) => u,
        Err(e) => {
            error!("DB error ensuring user: {}", e);
            bot.send_message(msg.chat.id, "Internal database error.").await?;
            return Ok(());
        }
    };

    match cmd {
        Command::Start => {
            if user.is_admin || user.is_whitelisted {
                // 1. Ambil Nama User dari Telegram
                let first_name = msg.from
                    .as_ref()
                    .map(|f| f.first_name.clone())
                    .unwrap_or_else(|| "pengguna".to_string());

                // 2. Hitung Waktu Awal Bulan Ini (Tanggal 1, Jam 00:00)
                let now = Utc::now();
                let start_of_month = Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                    .unwrap();

                // 3. Ambil Total Pengeluaran Bulan Ini
                let monthly_total = state.repo.get_user_expenses_since(user.id, start_of_month)
                    .await
                    .unwrap_or(0.0);

                // 4. Buat Tombol Inline (Menu)
                let buttons = vec![
                    vec![
                        InlineKeyboardButton::callback("📊 Laporan Harian", "report_daily"),
                        InlineKeyboardButton::callback("📊 Laporan Mingguan", "report_weekly"),
                    ],
                    vec![
                        InlineKeyboardButton::callback("📝 Tambah Pengeluaran", "add_expense"),
                        InlineKeyboardButton::callback("⚙️ Pengaturan", "settings"),
                    ],
                ];
                let keyboard = InlineKeyboardMarkup::new(buttons);

                // 5. Rangkai Pesan Dashboard dengan Format HTML
                let text = format!(
                    "Halo, <b>{}</b>! Selamat datang di Dashboard Keuanganmu.\n\n\
                    💰 <b>Total Pengeluaran Bulan Ini:</b> Rp {:.2}\n\n\
                    Apa yang ingin kamu pantau hari ini?",
                    first_name, monthly_total
                );

                bot.send_message(msg.chat.id, text)
                    .reply_markup(keyboard)
                    .parse_mode(ParseMode::Html)
                    .await?;

            } else {
                bot.send_message(msg.chat.id, "Welcome! You are not whitelisted yet. Please contact the administrator.").await?;
            }
        }
        Command::Whitelist(id_to_whitelist) => {
            if !user.is_admin {
                bot.send_message(msg.chat.id, "You are not authorized to use this command.").await?;
                return Ok(());
            }
            match state.repo.ensure_user(id_to_whitelist).await {
                Ok(_) => {
                    if let Err(e) = state.repo.set_whitelist(id_to_whitelist, true).await {
                        bot.send_message(msg.chat.id, format!("Failed: {}", e)).await?;
                    } else {
                        bot.send_message(msg.chat.id, format!("User {} is now whitelisted.", id_to_whitelist)).await?;
                    }
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Error finding user: {}", e)).await?;
                }
            }
        }
        Command::Blacklist(id_to_blacklist) => {
            if !user.is_admin {
                bot.send_message(msg.chat.id, "You are not authorized to use this command.").await?;
                return Ok(());
            }
            if let Err(e) = state.repo.set_whitelist(id_to_blacklist, false).await {
                bot.send_message(msg.chat.id, format!("Failed: {}", e)).await?;
            } else {
                bot.send_message(msg.chat.id, format!("User {} is now blacklisted.", id_to_blacklist)).await?;
            }
        }
        Command::Report(period) => {
            if !user.is_whitelisted {
                bot.send_message(msg.chat.id, "You are not whitelisted.").await?;
                return Ok(());
            }

            let since = match period.to_lowercase().as_str() {
                "daily" | "hari ini" => {

                    Utc::now() - chrono::Duration::days(1)
                },
                "weekly" | "mingguan" => Utc::now() - chrono::Duration::days(7),
                "monthly" | "bulanan" => Utc::now() - chrono::Duration::days(30),
                _ => Utc::now() - chrono::Duration::days(30)
            };

            match state.repo.get_user_expenses_since(user.id, since).await {
                Ok(total) => {
                    bot.send_message(msg.chat.id, format!("Total expenses for the selected period: Rp {}", total)).await?;
                }
                Err(e) => {
                    bot.send_message(msg.chat.id, format!("Error calculating report: {}", e)).await?;
                }
            }
        }
    };

    Ok(())
}

async fn message_handler(
    bot: Bot,
    msg: Message,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    let telegram_id = msg.chat.id.0;
    
    let user = match state.repo.ensure_user(telegram_id).await {
        Ok(u) => u,
        Err(_) => return Ok(()),
    };

    if !user.is_admin && !user.is_whitelisted {
        // Silently ignore or warn
        bot.send_message(msg.chat.id, "You are not whitelisted to record expenses.").await?;
        return Ok(());
    }

    if let Some(text) = msg.text() {
        let mut parser = state.parser.lock().await;
        let parsed = match parser.parse(text) {
            Ok(p) => p,
            Err(e) => {
                bot.send_message(msg.chat.id, format!("Failed to parse message: {}", e)).await?;
                return Ok(());
            }
        };

        if parsed.amount == 0.0 {
            bot.send_message(msg.chat.id, "Sorry, I couldn't detect an amount.").await?;
            return Ok(());
        }

        match state.repo.add_expense(user.id, parsed.amount, &parsed.description).await {
            Ok(_) => {
                bot.send_message(
                    msg.chat.id,
                    format!("Recorded! Item: {}, Amount: Rp {}", parsed.description.trim(), parsed.amount)
                ).await?;
            }
            Err(e) => {
                bot.send_message(msg.chat.id, format!("Database error: {}", e)).await?;
            }
        }
    }

    Ok(())
}
