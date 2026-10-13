# syntax=docker/dockerfile:1

# ---- Tahap 1: bangun paket WASM (polify-wasm) ----
FROM rust:1-bookworm AS wasm
RUN rustup target add wasm32-unknown-unknown \
 && curl -sSfL https://rustwasm.github.io/wasm-pack/installer/init.sh | sh
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# polify-core memuat config/portal-profiles.json lewat include_str!
COPY config ./config
RUN cd crates/polify-wasm && wasm-pack build --target web --release

# ---- Tahap 2: bangun situs statis (Astro) ----
FROM node:22-bookworm-slim AS web
WORKDIR /src/apps/web
COPY apps/web/package.json apps/web/package-lock.json ./
RUN npm ci
COPY apps/web ./
# Paket WASM dari tahap 1; sync-wasm.mjs membacanya dari ../../crates/polify-wasm/pkg
COPY --from=wasm /src/crates/polify-wasm/pkg /src/crates/polify-wasm/pkg
# Domain situs (dipakai astro.config.mjs untuk sitemap & canonical). Bisa di-override di Coolify.
ARG PUBLIC_SITE_URL=https://polify.cianjur.space
ENV PUBLIC_SITE_URL=${PUBLIC_SITE_URL}
RUN npm run build

# ---- Tahap 3: sajikan hasil build dengan Nginx ----
FROM nginx:1.27-alpine AS runtime
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=web /src/apps/web/dist /usr/share/nginx/html
EXPOSE 80
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD wget -qO- http://127.0.0.1/ >/dev/null 2>&1 || exit 1
CMD ["nginx", "-g", "daemon off;"]
