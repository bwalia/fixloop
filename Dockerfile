# syntax=docker/dockerfile:1
# Fixloop server image: a static-ish Rust binary on distroless, run as non-root.

FROM rust:1.94-bookworm AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p fixloop-cli \
 && cp target/release/fixloop /fixloop \
 && mkdir /data

FROM gcr.io/distroless/cc-debian12:nonroot
LABEL org.opencontainers.image.source="https://github.com/bwalia/fixloop" \
      org.opencontainers.image.description="Fixloop: review, find, file, fix, check, close" \
      org.opencontainers.image.licenses="Apache-2.0"
COPY --from=build /fixloop /usr/local/bin/fixloop
# Owned by the runtime user so a fresh volume mounted here is writable.
COPY --from=build --chown=65532:65532 /data /var/lib/fixloop
USER nonroot:nonroot
# Environment beats the config file, so only set what every container needs:
# listening on all interfaces instead of the 127.0.0.1 default.
ENV FIXLOOP_SERVER__BIND=0.0.0.0:8080
VOLUME ["/var/lib/fixloop"]
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/fixloop"]
CMD ["serve"]
