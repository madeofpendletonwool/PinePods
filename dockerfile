# Builder stage for compiling the Yew application
FROM rust:alpine AS builder
# Install build dependencies
RUN apk update && apk upgrade && \
    apk add --no-cache musl-dev libffi-dev zlib-dev jpeg-dev
RUN apk update && apk upgrade
# Add the Edge Community repository
RUN echo "@edge http://dl-cdn.alpinelinux.org/alpine/edge/community" >> /etc/apk/repositories
# Update the package index
RUN apk update
# Install the desired package from the edge community repository
RUN apk add trunk@edge
# Install wasm target and build tools
RUN rustup target add wasm32-unknown-unknown && \
    cargo install wasm-bindgen-cli
# Add application files to the builder stage
COPY ./web/Cargo.lock ./web/Cargo.toml ./web/dev-info.md ./web/index.html ./web/tailwind.config.js ./web/Trunk.toml /app/
COPY ./web/src /app/src
COPY ./web/static /app/static
WORKDIR /app
# Build the Yew application in release mode
RUN RUSTFLAGS="--cfg=web_sys_unstable_apis --cfg getrandom_backend=\"wasm_js\"" trunk build --features server_build --release

# Go builder stage for the gpodder API
FROM golang:alpine AS go-builder
WORKDIR /gpodder-api

# Install build dependencies
RUN apk add --no-cache git

# Copy go module files first for better layer caching
COPY ./gpodder-api/go.mod ./gpodder-api/go.sum ./
RUN go mod download

# Copy the rest of the source code
COPY ./gpodder-api/cmd ./cmd
COPY ./gpodder-api/config ./config
COPY ./gpodder-api/internal ./internal

# Build the application
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o gpodder-api ./cmd/server/

# Python builder stage for database setup
FROM python:3.11-alpine AS python-builder
WORKDIR /build

# Install build dependencies for PyInstaller and MariaDB connector
RUN apk add --no-cache gcc musl-dev libffi-dev openssl-dev mariadb-connector-c-dev

# Copy Python source files
COPY ./database_functions ./database_functions
COPY ./startup/setup_database_new.py ./startup/setup_database_new.py
COPY ./requirements.txt ./requirements.txt

# Install Python dependencies including PyInstaller
RUN pip install --no-cache-dir -r requirements.txt pyinstaller

# Build standalone database setup binary
RUN pyinstaller --onefile \
    --name pinepods-db-setup \
    --hidden-import psycopg \
    --hidden-import mysql.connector \
    --hidden-import cryptography \
    --hidden-import cryptography.fernet \
    --hidden-import passlib \
    --hidden-import passlib.hash \
    --hidden-import passlib.hash.argon2 \
    --hidden-import argon2 \
    --hidden-import argon2.exceptions \
    --hidden-import argon2.profiles \
    --hidden-import argon2._password_hasher \
    --add-data "database_functions:database_functions" \
    --console \
    startup/setup_database_new.py

# Rust API builder stage
FROM rust:alpine AS rust-api-builder
WORKDIR /rust-api

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

# Copy Rust API files
COPY ./rust-api/Cargo.toml ./rust-api/Cargo.lock ./
COPY ./rust-api/src ./src

# Set environment for static linking
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include

# Build the Rust API
RUN cargo build --release && strip target/release/pinepods-api

# Final stage for setting up runtime environment
FROM alpine
# Metadata
LABEL maintainer="Collin Pendleton <collinp@collinpendleton.com>"
# Install runtime dependencies
RUN apk add --no-cache tzdata nginx openssl bash mariadb-client postgresql-client curl ffmpeg wget jq mariadb-connector-c-dev


# Download and install latest yt-dlp binary (musllinux for Alpine)
RUN LATEST_VERSION=$(curl -s https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest | jq -r .tag_name) && \
    wget -O /usr/local/bin/yt-dlp "https://github.com/yt-dlp/yt-dlp/releases/download/${LATEST_VERSION}/yt-dlp_musllinux" && \
    chmod +x /usr/local/bin/yt-dlp

# Download and install Horust (x86_64)
RUN wget -O /tmp/horust.tar.gz "https://github.com/FedericoPonzi/Horust/releases/download/v0.1.7/horust-x86_64-unknown-linux-musl.tar.gz" && \
    cd /tmp && tar -xzf horust.tar.gz && \
    mv horust /usr/local/bin/ && \
    chmod +x /usr/local/bin/horust && \
    rm -f /tmp/horust.tar.gz

ENV TZ=UTC
# Copy compiled database setup binary (replaces Python dependency)
COPY --from=python-builder /build/dist/pinepods-db-setup /usr/local/bin/
# Copy built files from the builder stage to the Nginx serving directory
COPY --from=builder /app/dist /var/www/html/
# Copy translation files for the Rust API to access
COPY ./web/src/translations /var/www/html/static/translations
# Copy Go API binary from the go-builder stage
COPY --from=go-builder /gpodder-api/gpodder-api /usr/local/bin/
# Copy Rust API binary from the rust-api-builder stage
COPY --from=rust-api-builder /rust-api/target/release/pinepods-api /usr/local/bin/
# Move to the root directory to execute the startup script
WORKDIR /
# Copy startup scripts
COPY startup/startup.sh /startup.sh
RUN chmod +x /startup.sh
# Copy Pinepods runtime files
RUN mkdir -p /pinepods
RUN mkdir -p /var/log/pinepods/ && mkdir -p /etc/horust/services/
COPY startup/ /pinepods/startup/
# Legacy cron scripts removed - background tasks now handled by internal Rust scheduler
COPY clients/ /pinepods/clients/
COPY database_functions/ /pinepods/database_functions/
RUN chmod +x /pinepods/startup/startup.sh
ENV APP_ROOT=/pinepods
# Define the build argument
ARG PINEPODS_VERSION
# Write the Pinepods version to the current_version file
RUN echo "${PINEPODS_VERSION}" > /pinepods/current_version
# Configure Nginx
COPY startup/nginx.conf /etc/nginx/nginx.conf

# Copy script to start gpodder API
COPY ./gpodder-api/start-gpodder.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/start-gpodder.sh

RUN cp /usr/share/zoneinfo/UTC /etc/localtime && \
    echo "UTC" > /etc/timezone

# Expose ports
EXPOSE 8080 8000

# Start everything using the startup script
ENTRYPOINT ["bash", "/startup.sh"]
