# Builder stage for compiling the Yew application
FROM rust:alpine3.19 as builder

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

# Add your application files to the builder stage
COPY ./web /app
WORKDIR /app

# Build the Yew application in release mode
RUN trunk build --features server_build --release

# Final stage for setting up runtime environment
FROM alpine:3.19

# Metadata
LABEL maintainer="Collin Pendleton <collinp@collinpendleton.com>"

# Install runtime dependencies
RUN apk add --no-cache nginx python3 openssl py3-pip bash mariadb-client postgresql-client curl cronie openrc supervisor

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
COPY clients/ /pinepods/clients/
COPY database_functions/ /pinepods/database_functions/
RUN chmod +x /pinepods/startup/startup.sh

ENV APP_ROOT /pinepods

# Write the Pinepods version to the current_version file
RUN echo "${PINEPODS_VERSION}" > /pinepods/current_version

# Configure Nginx
COPY startup/nginx.conf /etc/nginx/nginx.conf

# Start Nginx and keep it running
# CMD ["nginx", "-g", "daemon off;"]

ENTRYPOINT ["bash", "/startup.sh"]
