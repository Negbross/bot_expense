# ==========================================
# STAGE 1: Chef (Instalasi Alat)
# ==========================================
FROM rust:1.88-slim-bookworm AS chef
WORKDIR /app

# Install dependensi sistem yang dibutuhkan crate (reqwest, ort, dll)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    g++ \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-chef
RUN cargo install cargo-chef

# ==========================================
# STAGE 2: Planner (Menghitung Resep)
# ==========================================
FROM chef AS planner
COPY . .
# Membaca Cargo.toml dan membuat daftar presisi dari semua dependensi
RUN cargo chef prepare --recipe-path recipe.json

# ==========================================
# STAGE 3: Builder (Kompilasi Super Cepat)
# ==========================================
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Build DEPENDENSI saja. Layer ini akan di-cache permanen oleh Podman
# selama kamu tidak mengubah Cargo.toml atau Cargo.lock
RUN cargo chef cook --release --recipe-path recipe.json

# Baru salin kode sumber Rust milikmu (src/, assets/, migrations/)
COPY . .

# Build aplikasi utama (sekarang sangat cepat karena dependensi sudah di-cache)
RUN cargo build --release

# ==========================================
# STAGE 4: Runtime (Image Produksi Super Ringan)
# ==========================================
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install dependensi runtime agar bot bisa mengakses internet (HTTPS)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Pindahkan file binary yang sudah jadi ke lingkungan produksi
COPY --from=builder /app/target/release/my_expense_bot /usr/local/bin/my_expense_bot

# Pindahkan model AI ONNX agar bisa dibaca oleh bot
COPY --from=builder /app/assets /app/assets

# Jalankan bot
CMD ["my_expense_bot"]