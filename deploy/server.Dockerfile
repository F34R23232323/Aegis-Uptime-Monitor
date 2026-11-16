FROM scratch AS source
COPY Cargo.toml Cargo.lock /
COPY crates /crates
COPY migrations /migrations
FROM rust:1.100-bookworm AS build
WORKDIR /build
COPY --from=source / /build/
RUN --mount=type=secret,id=build_ca \
    if [ -f /run/secrets/build_ca ]; then export CARGO_HTTP_CAINFO=/run/secrets/build_ca; fi; \
    cargo build --locked --release -p aegis-server -p aegis-agent
FROM debian:bookworm-slim AS server
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 aegis
COPY --from=build /build/target/release/aegis-server /usr/local/bin/
USER aegis
EXPOSE 3001
HEALTHCHECK --interval=15s --timeout=3s CMD curl -fsS http://127.0.0.1:3001/healthz || exit 1
ENTRYPOINT ["aegis-server"]
FROM debian:bookworm-slim AS agent
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 aegis
COPY --from=build /build/target/release/aegis-agent /usr/local/bin/
USER aegis
ENTRYPOINT ["aegis-agent"]
