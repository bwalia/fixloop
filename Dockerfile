# syntax=docker/dockerfile:1
# Fixloop server image: a static-ish Rust binary on distroless, run as non-root.

FROM rust:1.94-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p fixloop-cli \
 && cp target/release/fixloop /fixloop

FROM gcr.io/distroless/cc-debian12:nonroot
LABEL org.opencontainers.image.source="https://github.com/bwalia/fixloop" \
      org.opencontainers.image.description="Fixloop: review, find, file, fix, check, close" \
      org.opencontainers.image.licenses="Apache-2.0"
COPY --from=build /fixloop /usr/local/bin/fixloop
USER nonroot:nonroot
ENV FIXLOOP_SERVER__BIND=0.0.0.0:8080 \
    FIXLOOP_TELEMETRY__LOG_FORMAT=json
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/fixloop"]
CMD ["serve"]
