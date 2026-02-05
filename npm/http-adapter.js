#!/usr/bin/env node

const http = require("http");
const { spawn } = require("child_process");
const { getBinaryPath } = require("./index.js");

const DEFAULT_PORT = 3541;
const DEFAULT_TIMEOUT_MS = 30000;

function parseArgs(argv) {
  const args = argv.slice(2);
  let port = process.env.PORT ? Number(process.env.PORT) : DEFAULT_PORT;
  let binaryArgs = [];

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--port" && args[i + 1]) {
      port = Number(args[i + 1]);
      i += 1;
      continue;
    }
    if (arg === "--") {
      binaryArgs = args.slice(i + 1);
      break;
    }
  }

  const envArgs = process.env.MCP_STDIO_ARGS
    ? process.env.MCP_STDIO_ARGS.split(" ").filter(Boolean)
    : [];

  return {
    port,
    binaryArgs: binaryArgs.length > 0 ? binaryArgs : envArgs,
  };
}

function encodeMessage(message) {
  const json = JSON.stringify(message);
  const length = Buffer.byteLength(json, "utf8");
  return `Content-Length: ${length}\r\n\r\n${json}`;
}

function createStdioParser(onMessage) {
  let buffer = Buffer.alloc(0);

  return (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);

    while (true) {
      const headerEnd = buffer.indexOf("\r\n\r\n");
      if (headerEnd === -1) {
        break;
      }

      const header = buffer.slice(0, headerEnd).toString("utf8");
      const match = header.match(/Content-Length:\s*(\d+)/i);
      if (!match) {
        buffer = buffer.slice(headerEnd + 4);
        continue;
      }

      const length = Number(match[1]);
      const messageStart = headerEnd + 4;
      const messageEnd = messageStart + length;

      if (buffer.length < messageEnd) {
        break;
      }

      const payload = buffer.slice(messageStart, messageEnd).toString("utf8");
      buffer = buffer.slice(messageEnd);

      try {
        const json = JSON.parse(payload);
        onMessage(json);
      } catch (err) {
        console.error("❌ Failed to parse MCP message:", err.message);
      }
    }
  };
}

function startServer() {
  const { port, binaryArgs } = parseArgs(process.argv);
  const timeoutMs = process.env.MCP_HTTP_TIMEOUT_MS
    ? Number(process.env.MCP_HTTP_TIMEOUT_MS)
    : DEFAULT_TIMEOUT_MS;

  const command = process.env.MCP_STDIO_COMMAND || getBinaryPath();
  const child = spawn(command, binaryArgs, {
    stdio: ["pipe", "pipe", "pipe"],
    env: process.env,
  });

  child.on("error", (err) => {
    console.error("❌ Failed to start MCP stdio server:", err.message);
    process.exit(1);
  });

  child.stderr.on("data", (data) => {
    process.stderr.write(data.toString());
  });

  const pending = new Map();
  const sseClients = new Set();

  const handleMessage = (message) => {
    if (message && Object.prototype.hasOwnProperty.call(message, "id")) {
      const pendingRequest = pending.get(message.id);
      if (pendingRequest) {
        pending.delete(message.id);
        pendingRequest.resolve(message);
      }
    }

    const payload = `event: message\ndata: ${JSON.stringify(message)}\n\n`;
    for (const res of sseClients) {
      res.write(payload);
    }
  };

  child.stdout.on("data", createStdioParser(handleMessage));

  const server = http.createServer((req, res) => {
    const { method, url } = req;

    if (method === "GET" && url === "/health") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ status: "ok" }));
      return;
    }

    if (method === "GET" && url === "/sse") {
      res.writeHead(200, {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        Connection: "keep-alive",
        "Access-Control-Allow-Origin": "*",
      });
      res.write("event: ready\ndata: {}\n\n");
      sseClients.add(res);

      req.on("close", () => {
        sseClients.delete(res);
      });
      return;
    }

    if (method === "POST" && url === "/rpc") {
      let body = "";
      req.on("data", (chunk) => {
        body += chunk.toString();
      });

      req.on("end", () => {
        let payload;
        try {
          payload = JSON.parse(body);
        } catch (err) {
          res.writeHead(400, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "Invalid JSON payload" }));
          return;
        }

        const message = encodeMessage(payload);
        child.stdin.write(message);

        if (!Object.prototype.hasOwnProperty.call(payload, "id")) {
          res.writeHead(202, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ status: "accepted" }));
          return;
        }

        const timeout = setTimeout(() => {
          pending.delete(payload.id);
          res.writeHead(504, { "Content-Type": "application/json" });
          res.end(JSON.stringify({ error: "MCP response timeout" }));
        }, timeoutMs);

        pending.set(payload.id, {
          resolve: (response) => {
            clearTimeout(timeout);
            res.writeHead(200, { "Content-Type": "application/json" });
            res.end(JSON.stringify(response));
          },
        });
      });
      return;
    }

    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "Not Found" }));
  });

  server.listen(port, () => {
    console.log(`✅ MCP HTTP/SSE adapter listening on http://localhost:${port}`);
    console.log("- POST /rpc for JSON-RPC calls");
    console.log("- GET  /sse for server-sent events");
  });

  const shutdown = () => {
    server.close(() => {
      child.kill("SIGTERM");
      process.exit(0);
    });
  };

  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}

startServer();