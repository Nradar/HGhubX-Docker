FROM node:24-bookworm-slim AS frontend
WORKDIR /build
RUN npm install --global pnpm@11.25.0
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml .npmrc ./
RUN pnpm install --frozen-lockfile
COPY index.html tsconfig.json vite.config.ts ./
COPY public ./public
COPY src ./src
RUN pnpm run build:web

FROM rust:1.95-bookworm AS backend
WORKDIR /build
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs ./
COPY src-tauri/src ./src
RUN cargo build --locked --release --no-default-features --features web --bin hghubx-web

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && install -d -o 10001 -g 10001 -m 0700 /data/state
WORKDIR /app
COPY --from=backend /build/target/release/hghubx-web /usr/local/bin/hghubx-web
COPY --from=frontend /build/dist ./dist
COPY LICENSE ./
ENV HGHUBX_WEB_ASSETS=/app/dist HGHUBX_WEB_DATA=/data/state
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:8080/healthz"]
ENTRYPOINT ["hghubx-web"]
