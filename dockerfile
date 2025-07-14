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
RUN CGO_ENABLED=0 GOOS=linux go build -o gpodder-api ./cmd/server/

# Rust API builder stage
FROM rust:alpine AS rust-api-builder
WORKDIR /rust-api

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev

# Copy Rust API files
COPY ./rust-api/Cargo.toml ./rust-api/Cargo.lock ./
COPY ./rust-api/src ./src

# Build the Rust API
RUN cargo build --release

# Final stage for setting up runtime environment
FROM alpine
# Metadata
LABEL maintainer="Collin Pendleton <collinp@collinpendleton.com>"
# Install runtime dependencies
RUN apk add --no-cache tzdata nginx python3 openssl py3-pip bash mariadb-client postgresql-client curl cronie openrc ffmpeg supervisor
ENV TZ=UTC
# Setup Python environment
RUN python3 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
# Install Python packages
COPY ./requirements.txt /
RUN pip install --no-cache-dir -r /requirements.txt
# Copy wait-for-it script and give execute permission
COPY ./wait-for-it/wait-for-it.sh /wait-for-it.sh
RUN chmod +x /wait-for-it.sh
# Copy built files from the builder stage to the Nginx serving directory
COPY --from=builder /app/dist /var/www/html/
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
RUN mkdir -p /var/log/supervisor/
COPY startup/ /pinepods/startup/
RUN chmod +x /pinepods/startup/call_refresh_endpoint.sh
RUN chmod +x /pinepods/startup/app_startup.sh
RUN chmod +x /pinepods/startup/call_nightly_tasks.sh
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
