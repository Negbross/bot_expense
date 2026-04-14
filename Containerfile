# ==========================================
# STAGE 1: Builder (Kompilasi)
# ==========================================
FROM rust:1.84-slim-bookworm as builder

WORKDIR /app

# Install dependensi sistem untuk kompilasi (OpenSSL & PKG Config)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Salin manifest terlebih dahulu untuk caching dependencies
COPY Cargo.toml Cargo.lock ./
# Buat dummy main untuk pre-build dependencies (opsional tapi mempercepat build ulang)
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

# Salin kode sumber asli dan folder assets (untuk model ONNX)
COPY . .

# Build aplikasi dalam mode release
RUN cargo build --release

# ==========================================
# STAGE 2: Runtime (Produksi)
# ==========================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies yang dibutuhkan:
# - ca-certificates: Agar bot bisa melakukan request HTTPS ke Telegram & API Kurs
# - libssl3: Untuk keperluan enkripsi/SSL
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Salin binary dari stage builder
COPY --from=builder /app/target/release/my_expense_bot /usr/local/bin/my_expense_bot

# Salin folder assets agar model AI ONNX bisa diakses saat runtime
COPY --from=builder /app/assets /app/assets

# Jalankan bot
CMD ["my_expense_bot"]