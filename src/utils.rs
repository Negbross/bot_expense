use std::collections::HashMap;
use serde::Deserialize;
use teloxide::types::User;

pub fn get_text(key: &str, user: Option<&User>, name: &str, amount: f64, exchange_rate: f64) -> String {
    // Deteksi jika bahasa berawalan "id" (contoh: "id" atau "id-ID")
    let lang_code = user
        .and_then(|u| u.language_code.as_deref())
        .unwrap_or("en"); // Default ke English
        
    let is_id = lang_code.starts_with("id");

    let formatted_money = if is_id {
        format!("Rp {:.0}", amount)
    } else {
        format!("${:.2}", amount / exchange_rate)
    };

    match key {
        "err_db" => if is_id { "Terjadi kesalahan database internal." } else { "Internal database error." }.to_string(),
        "not_whitelisted" => if is_id { 
            "⛔️ <b>Akses Ditolak</b>\n\nHalo! Kamu belum terdaftar di sistem. Silakan klik tombol di bawah ini untuk meminta akses kepada Administrator.".to_string()
        } else { 
            "⛔️ <b>Access Denied</b>\n\nHello! You are not whitelisted yet. Please click the button below to request access from the Administrator.".to_string() 
        },
        "not_authorized" => if is_id { "Kamu tidak memiliki izin." } else { "You are not authorized." }.to_string(),
        "dashboard_text" => if is_id {
            format!(
                "Halo, <b>{}</b>! Selamat datang di Dashboard Keuanganmu.\n\n\
                💰 <b>Total Pengeluaran Bulan Ini:</b> Rp {:.2}\n\n\
                Apa yang ingin kamu pantau hari ini?", name, amount
            )
        } else {
            format!(
                "Hello, <b>{}</b>! Welcome to your Financial Dashboard.\n\n\
                💰 <b>Total Expenses This Month:</b> Rp {:.2}\n\n\
                What would you like to track today?", name, amount
            )
        },
        // Tombol-tombol menu
        "btn_back" => if is_id { "🔙 Kembali" } else { "🔙 Back" }.to_string(),
        "btn_daily" => if is_id { "📊 Laporan Harian" } else { "📊 Daily Report" }.to_string(),
        "btn_weekly" => if is_id { "📊 Laporan Mingguan" } else { "📊 Weekly Report" }.to_string(),
        "btn_request" => if is_id { "💬 Chat Administrator" } else { "💬 Contact Admin" }.to_string(),
        
        "report_daily" => if is_id {
            format!("📊 <b>Laporan Harian</b>\n\nTotal Pengeluaran: {}", formatted_money)
        } else {
            format!("📊 <b>Daily Report</b>\n\nTotal Expenses: {}", formatted_money)
        },
        "report_weekly" => if is_id {
            format!("📊 <b>Laporan Mingguan (7 Hari Terakhir)</b>\n\nTotal Pengeluaran: {}", formatted_money)
        } else {
            format!("📊 <b>Weekly Report (Last 7 Days)</b>\n\nTotal Expenses: {}", formatted_money)
        },
        "add_expense_prompt" => if is_id {
            "Ketikkan pengeluaranmu secara langsung. Contoh: 'Beli kopi 25rb'".to_string()
        } else {
            "Type your expense directly. Example: 'Bought coffee 25 bucks'".to_string()
        },
        _ => format!("Missing key: {}", key),
    }
}

#[derive(Deserialize)]
struct ExchangeResponse {
    rates: HashMap<String, f64>,
}

// Fungsi ini mengambil kurs 1 USD ke IDR
pub async fn fetch_exchange_rate() -> f64 {
    let url = "https://open.er-api.com/v6/latest/USD";
    
    // Coba tembak API
    if let Ok(resp) = reqwest::get(url).await {
        if let Ok(data) = resp.json::<ExchangeResponse>().await {
            // Jika berhasil, ambil nilai kurs IDR
            if let Some(rate) = data.rates.get("IDR") {
                println!("✅ Berhasil mengambil Exchange Rate: 1 USD = Rp {}", rate);
                return *rate;
            }
        }
    }
    
    // Fallback aman jika API error atau tidak ada internet
    println!("⚠️ Gagal mengambil API, menggunakan rate fallback (16000).");
    16000.0 
}