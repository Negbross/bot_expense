# ==========================================
# STAGE 1: Chef (Instalasi Alat di OS Baru)
# ==========================================
FROM ubuntu:24.04 AS chef
WORKDIR /app

# Install compiler C++, pkg-config, OpenSSL, dan curl
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust & Cargo terbaru secara otomatis
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN cargo install cargo-chef

# ==========================================
# STAGE 2: Planner (Menghitung Resep)
# ==========================================
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ==========================================
# STAGE 3: Builder (Kompilasi Super Cepat)
# ==========================================
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release

# ==========================================
# STAGE 4: Runtime (Lingkungan Produksi Ubuntu)
# ==========================================
FROM ubuntu:24.04 AS runtime
WORKDIR /app

# Install sertifikat internet dan OpenSSL
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y \
    ca-certificates \
    openssl \
    && rm -rf /var/lib/apt/lists/*

# Salin binary bot_expense yang sudah jadi
COPY --from=builder /app/target/release/bot_expense /usr/local/bin/bot_expense

# Salin folder AI
COPY --from=builder /app/assets /app/assets

CMD ["bot_expense"]