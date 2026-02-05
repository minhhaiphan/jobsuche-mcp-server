FROM rust:1.88 as builder

# Install OpenSSL development libraries for native-tls
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN cargo build --release --package jobsuche-mcp-server

FROM node:20-bookworm-slim

# Install CA certificates and OpenSSL for native-tls runtime
RUN apt-get update && apt-get install -y ca-certificates libssl3 \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/jobsuche-mcp-server /usr/local/bin/jobsuche-mcp-server
COPY npm/http-adapter.js /usr/local/bin/mcp-http-adapter.js

ENV MCP_STDIO_COMMAND=/usr/local/bin/jobsuche-mcp-server
ENV PORT=3541

EXPOSE 3541
CMD ["node", "/usr/local/bin/mcp-http-adapter.js"]
