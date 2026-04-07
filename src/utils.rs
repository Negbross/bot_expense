pub fn get_text(key: &str, lang_code: &str, name: &str, amount: f64) -> String {
    // Deteksi jika bahasa berawalan "id" (contoh: "id" atau "id-ID")
    let is_id = lang_code.starts_with("id");

    match key {
        "err_db" => if is_id { "Terjadi kesalahan database internal." } else { "Internal database error." }.to_string(),
        "not_whitelisted" => if is_id {
            "Selamat datang! Kamu belum terdaftar. Silakan hubungi administrator."
        } else {
            "Welcome! You are not whitelisted yet. Please contact the administrator."
        }.to_string(),
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
        "btn_daily" => if is_id { "📊 Laporan Harian" } else { "📊 Daily Report" }.to_string(),
        "btn_weekly" => if is_id { "📊 Laporan Mingguan" } else { "📊 Weekly Report" }.to_string(),
        "btn_add" => if is_id { "📝 Tambah Pengeluaran" } else { "📝 Add Expense" }.to_string(),
        "btn_settings" => if is_id { "⚙️ Pengaturan" } else { "⚙️ Settings" }.to_string(),
        _ => "Text missing".to_string(),
    }
}