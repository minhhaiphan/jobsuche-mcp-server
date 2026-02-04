FROM rust:1.88 as builder

WORKDIR /app
COPY . .

RUN cargo build --release --package jobsuche-mcp-server

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/jobsuche-mcp-server /usr/local/bin/jobsuche-mcp-server

EXPOSE 3000
CMD ["jobsuche-mcp-server"]
