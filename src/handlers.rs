use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use std::sync::Arc;
use chrono::{Datelike, TimeZone, Utc};
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tracing::error;

use crate::repos::Repository;
use crate::parser::ExpenseParser;
use crate::utils::{self, get_text};
use crate::state::AppState;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "These commands are supported:")]
pub enum Command {
    #[command(description = "Start the bot and request access.")]
    Start,
    #[command(description = "Admin only: Whitelist a user by telegram ID.")]
    Whitelist(String),
    #[command(description = "Admin only: Blacklist a user by telegram ID.")]
    Blacklist(String),
    #[command(description = "Report expenses (e.g. /report daily, /report weekly, /report monthly).")]
    Report(String),
}

pub async fn run_bot(repo: Repository, parser: ExpenseParser) {
    let bot = Bot::from_env();
    let live_rate = utils::fetch_exchange_rate().await;
    let state = Arc::new(AppState { repo, parser: tokio::sync::Mutex::new(parser), exchange_rate: live_rate });

    let handler = dptree::entry()
        // Cabang Utama 1: Semua yang berupa Pesan Masuk (Teks)
        .branch(
            Update::filter_message()
                .branch(dptree::entry().filter_command::<Command>().endpoint(command_handler))
                .branch(dptree::entry().endpoint(message_handler))
        )
        // Cabang Utama 2: Semua yang berupa Klik Tombol Inline
        .branch(
            Update::filter_callback_query()
                .endpoint(callback_handler) 
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
    
    // Ambil username si pengirim pesan (bisa None kalau dia gak pasang username di Telegram)
    let current_username = msg.from.as_ref().and_then(|u| u.username.clone())
        .unwrap_or("".to_string());
    
    // Lempar username ke repo agar disimpan/di-update
    let user = match state.repo.ensure_user(telegram_id, &current_username).await {
        Ok(u) => u,
        Err(_) => {
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
                        InlineKeyboardButton::callback(get_text("btn_daily", msg.from.as_ref(), "", 0.0, state.exchange_rate), "report_daily"),
                        InlineKeyboardButton::callback(get_text("btn_weekly", msg.from.as_ref(), "", 0.0, state.exchange_rate), "report_weekly"),
                    ],
                    vec![
                        InlineKeyboardButton::url(
                            "📝 Feedback",
                            reqwest::Url::parse("https://subbarscrap.my.id")
                                .unwrap()
                        )
                    ]
                ];
                let keyboard = InlineKeyboardMarkup::new(buttons);

                // 5. Rangkai Pesan Dashboard dengan Format HTML
                let text = get_text(
                    "dashboard_text",
                    msg.from.as_ref(),
                    first_name.as_str(),
                    monthly_total,
                    state.exchange_rate,
                );

                bot.send_message(msg.chat.id, text)
                    .reply_markup(keyboard)
                    .parse_mode(ParseMode::Html)
                    .await?;

            } else {
                // 1. Ambil teks penolakan dari kamus
                let text = get_text("not_whitelisted", msg.from.as_ref(), "", 0.0, state.exchange_rate);
                
                // 2. Buat URL menuju akun Telegram pribadimu
                if let Ok(url) = reqwest::Url::parse("https://t.me/miesub") {
                    
                    // 3. Rangkai tombol URL
                    let keyboard = InlineKeyboardMarkup::new(vec![vec![
                        InlineKeyboardButton::url(
                            get_text("btn_request", msg.from.as_ref(), "", 0.0, state.exchange_rate), 
                            url
                        )
                    ]]);

                    // 4. Kirim pesan beserta tombolnya
                    bot.send_message(msg.chat.id, text)
                        .reply_markup(keyboard)
                        .parse_mode(ParseMode::Html)
                        .await?;
                        
                } else {
                    // Fallback aman jika URL gagal di-parse (hanya kirim teks)
                    bot.send_message(msg.chat.id, text)
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
            }
        }
        Command::Whitelist(target) => {
            if !user.is_admin {
                bot.send_message(msg.chat.id, "You are not authorized to use this command.").await?;
                return Ok(());
            }
           // Coba ubah input menjadi angka murni (i64)
            if let Ok(target_id) = target.parse::<i64>() {
                
                // JALUR 1: ADMIN MENGGUNAKAN TELEGRAM ID (Angka)
                match state.repo.set_whitelist(target_id, true).await {
                    Ok(_) => {
                        bot.send_message(msg.chat.id, format!("Berhasil! User ID {} sekarang di-whitelist.", target_id)).await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error database: {}", e)).await?;
                    }
                }

            } else {
                
                // JALUR 2: ADMIN MENGGUNAKAN USERNAME (Ada huruf/simbol)
                let clean_username = target.trim_start_matches('@');
                match state.repo.set_whitelist_by_username(clean_username, true).await {
                    Ok(true) => {
                        bot.send_message(msg.chat.id, format!("Berhasil! User @{} sekarang di-whitelist.", clean_username)).await?;
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, format!("Gagal. User @{} tidak ditemukan di database. Pastikan ia sudah menekan /start.", clean_username)).await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error database: {}", e)).await?;
                    }
                }
            }
        }
        Command::Blacklist(target) => {
            if !user.is_admin {
                bot.send_message(msg.chat.id, "You are not authorized to use this command.").await?;
                return Ok(());
            }
            // Coba ubah input menjadi angka murni (i64)
            if let Ok(target_id) = target.parse::<i64>() {
                
                // JALUR 1: ADMIN MENGGUNAKAN TELEGRAM ID (Angka)
                match state.repo.set_whitelist(target_id, false).await {
                    Ok(_) => {
                        bot.send_message(msg.chat.id, format!("Berhasil! User ID {} sekarang di-blacklist.", target_id)).await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error database: {}", e)).await?;
                    }
                }

            } else {
                
                // JALUR 2: ADMIN MENGGUNAKAN USERNAME (Ada huruf/simbol)
                let clean_username = target.trim_start_matches('@');
                match state.repo.set_whitelist_by_username(clean_username, false).await {
                    Ok(true) => {
                        bot.send_message(msg.chat.id, format!("Berhasil! User @{} sekarang di-blacklist.", clean_username)).await?;
                    }
                    Ok(false) => {
                        bot.send_message(msg.chat.id, format!("Gagal. User @{} tidak ditemukan di database. Pastikan orang tersebut sudah mengeklik /start ke bot ini minimal 1 kali.", clean_username)).await?;
                    }
                    Err(e) => {
                        bot.send_message(msg.chat.id, format!("Error database: {}", e)).await?;
                    }
                }
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
    
    let user = match state.repo.ensure_user(telegram_id, &msg.from.as_ref().and_then(|u| u.username.clone()).unwrap_or("".to_string())).await {
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

        // 1. Jaring Pengaman untuk Nama Barang
        let safe_item_name = if parsed.item_name.trim().is_empty() {
            "Tanpa Nama".to_string()
        } else {
            parsed.item_name.clone()
        };

        // 2. Jaring Pengaman untuk Kategori
        let safe_category = if parsed.category_group.trim().is_empty() {
            "other".to_string() // Kategori default
        } else {
            parsed.category_group.clone()
        };

        match state.repo.add_expense(user.id, parsed.amount, &parsed.description, &safe_item_name, &safe_category).await {
            Ok(_) => {
                bot.send_message(
                    msg.chat.id,
                    format!("Recorded! Item Name: {}, Amount: Rp {}, Category: {}", parsed.item_name.trim(), parsed.amount, parsed.category_group.trim())
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

        // Ambil user dari database (mirip dengan di command_handler)
        let user = match state.repo.ensure_user(telegram_id as i64, &q.from.username.clone().unwrap_or("".to_string())).await {
            Ok(u) => u,
            Err(e) => {
                error!("DB error: {}", e);
                return Ok(());
            }
        };

        

        // Pastikan pesan tempat tombol itu menempel masih bisa diakses
        if let Some(msg) = q.regular_message() {
            let now = Utc::now();

            let back_button = vec![vec![InlineKeyboardButton::callback(
                get_text("btn_back", Some(&q.from), "", 0.0, state.exchange_rate),
                "back_to_dashboard",
            )]];
            let back_keyboard = InlineKeyboardMarkup::new(back_button);

            match data.as_str() {
                "report_daily" => {
                    let start_of_day = Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0).unwrap();
                    let total = state.repo.get_user_expenses_since(user.id, start_of_day).await.unwrap_or(0.0);

                    let text = get_text("report_daily", msg.from.as_ref(), "", total, state.exchange_rate);
                    // Edit pesan dashboard yang lama menjadi hasil laporan
                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(back_keyboard)
                        .await?;
                }
                "report_weekly" => {
                    let start_of_week = now - chrono::Duration::days(7);
                    let total = state.repo.get_user_expenses_since(user.id, start_of_week).await.unwrap_or(0.0);

                    let text = get_text("report_weekly", msg.from.as_ref(), "", total, state.exchange_rate);
                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(back_keyboard)
                        .await?;
                }
                "report_monthly" => {
                    let start_of_month = Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0).unwrap();
                    let total = state.repo.get_user_expenses_since(user.id, start_of_month).await.unwrap_or(0.0);

                    let text = get_text("report_monthly", msg.from.as_ref(), "", total, state.exchange_rate);
                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(back_keyboard)
                        .await?;
                }
                "feedback" => {

                }
                "back_to_dashboard" => {
                    let first_name = q.from.first_name.clone();
                    let start_of_month = Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0).unwrap();
                    let monthly_total = state.repo.get_user_expenses_since(user.id, start_of_month).await.unwrap_or(0.0);

                    let buttons = vec![
                        vec![
                            InlineKeyboardButton::callback(get_text("btn_daily", Some(&q.from), "", 0.0, state.exchange_rate), "report_daily"),
                            InlineKeyboardButton::callback(get_text("btn_weekly", Some(&q.from), "", 0.0, state.exchange_rate), "report_weekly"),
                        ],
                        vec![
                            InlineKeyboardButton::url(
                                "📝 Feedback",
                                reqwest::Url::parse("https://subbarscrap.my.id")
                                    .unwrap()
                            )
                        ]
                    ];
                    let keyboard = InlineKeyboardMarkup::new(buttons);
                    let text = get_text("dashboard_text", Some(&q.from), &first_name, monthly_total, state.exchange_rate);

                    bot.edit_message_text(msg.chat.id, msg.id, text)
                        .reply_markup(keyboard)
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                _ => {}
            }
        }
    }

    Ok(())
}