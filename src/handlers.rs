use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use std::sync::Arc;
use chrono::{Datelike, TimeZone, Utc};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tracing::error;

use crate::repos::Repository;
use crate::parser::ExpenseParser;
use crate::utils::get_text;

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
        ).branch(
            Update::filter_callback_query()
                .endpoint(callback_handler) // Arahkan ke fungsi baru kita
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

                let lang_code = msg.from
                    .as_ref()
                    .and_then(|l| l.language_code.clone())
                    .unwrap_or_else(|| "en".to_string());

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

async fn callback_handler(
    bot: Bot,
    q: CallbackQuery,
    state: Arc<AppState>,
) -> ResponseResult<()> {
    // 1. WAJIB: Jawab callback agar ikon loading di tombol Telegram berhenti berputar
    bot.answer_callback_query(q.id.clone()).await?;

    // Dapatkan data spesifik dari tombol yang ditekan
    if let Some(data) = q.data.as_ref() {
        let telegram_id = q.from.id.0;

        let lang_code = q.from.language_code.clone().unwrap_or_else(|| "en".to_string());

        // Ambil user dari database (mirip dengan di command_handler)
        let user = match state.repo.ensure_user(telegram_id as i64).await {
            Ok(u) => u,
            Err(e) => {
                error!("DB error: {}", e);
                return Ok(());
            }
        };

        // Pastikan pesan tempat tombol itu menempel masih bisa diakses
        if let Some(msg) = q.regular_message() {
            let now = Utc::now();

            match data.as_str() {
                "report_daily" => {
                    let start_of_day = Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0).unwrap();
                    let total = state.repo.get_user_expenses_since(user.id, start_of_day).await.unwrap_or(0.0);

                    let text = get_text("report_daily", &lang_code, "", total);
                    // Edit pesan dashboard yang lama menjadi hasil laporan
                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                "report_weekly" => {
                    let start_of_week = now - chrono::Duration::days(7);
                    let total = state.repo.get_user_expenses_since(user.id, start_of_week).await.unwrap_or(0.0);

                    let text = get_text("report_weekly", &lang_code, "", total);
                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                "add_expense" => {
                    let text = get_text("add_expense", &lang_code, "", 0.0);
                    bot.send_message(msg.chat.id, text).await?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}