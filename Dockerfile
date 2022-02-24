FROM rust:1.56 AS builder

WORKDIR /

WORKDIR /app

# Copy the source code.
COPY . ./

RUN cargo build --release

FROM ubuntu AS final

COPY --from=builder /app/target/release/agora /usr/local/bin/agora

# Make sure we use the virtualenv:
ENV PATH="/usr/local/bin:$PATH"

# Copy the entrypoint script.
COPY "entrypoint.sh" .
RUN chmod +x entrypoint.sh

CMD ["./entrypoint.sh"]
