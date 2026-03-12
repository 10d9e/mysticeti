FROM rust:slim-bullseye AS builder

WORKDIR /app

# Build full workspace in release mode
COPY . .
RUN cargo build --release -p mysticeti

FROM rust:slim-bullseye AS runtime

RUN useradd -m mysticeti
WORKDIR /home/mysticeti

COPY --from=builder /app/target/release/mysticeti /usr/local/bin/mysticeti
COPY --from=builder /app/target/release/mysticeti-client /usr/local/bin/mysticeti-client
COPY --from=builder /app/mysticeti/config /etc/mysticeti

USER mysticeti

VOLUME ["/var/lib/mysticeti"]

ENTRYPOINT ["/usr/local/bin/mysticeti"]
CMD ["--authority-index","0","--config","/etc/mysticeti/docker-committee.toml","--storage-dir","/var/lib/mysticeti"]

