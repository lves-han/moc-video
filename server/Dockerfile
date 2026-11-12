FROM rust:1.92-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/moc-core ./crates/moc-core
COPY server/Cargo.toml ./server/Cargo.toml
COPY server/src ./server/src
RUN cargo build --locked --release -p moc-service -j 2

FROM debian:bookworm-slim
COPY --from=build /build/target/release/moc-service /usr/local/bin/moc-service
ENV MOC_SERVICE_LISTEN=0.0.0.0:8787 MOC_SERVICE_DATA_DIR=/data
VOLUME ["/data"]
EXPOSE 8787
ENTRYPOINT ["moc-service"]
