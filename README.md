# AgentGuard

AgentGuard v0.2 is a local firewall and reversible execution layer for autonomous coding agents. The Rust daemon validates TypeScript and JavaScript syntax with tree-sitter, scans proposed content for credentials, executes commands in an isolated shadow copy with a 60-second timeout, exposes MCP tools over SSE, and broadcasts decisions to the React control plane over WebSockets.

## Requirements

- Rust 1.85 or newer
- Node.js 20 or newer
- npm 10 or newer

## Run

Start the daemon from the repository root:

```sh
cargo run -p agentguard -- start --workspace .
```

Start the control plane in another terminal:

```sh
npm --prefix frontend install
npm --prefix frontend run dev
```

Open `http://127.0.0.1:5173`. Vite proxies `/api` and `/ws` to the daemon on `127.0.0.1:8080`.

## Runtime policy

Read the active policy with `GET /api/policy` and update it with `PATCH /api/policy`:

```json
{
  "autoApproveAstSafe": true,
  "requireHumanForDeletions": true,
  "strictSecretScanning": true
}
```

The control plane syncs each toggle immediately. The daemon owns the authoritative runtime state.

Wrap a command in a shadow workspace:

```sh
cargo run -p agentguard -- run --workspace . npm test
```

The command runs in a temporary copy that excludes `.git`, `node_modules`, and Rust `target` directories. A zero exit status syncs changed and new files back. A non-zero status discards the shadow changes.

## API

### Validate syntax

```sh
curl -X POST http://127.0.0.1:8080/api/ast \
  -H "content-type: application/json" \
  -d '{"path":"src/example.ts","code":"const answer: number = 42;"}'
```

### Scan content

```sh
curl -X POST http://127.0.0.1:8080/api/secrets \
  -H "content-type: application/json" \
  -d '{"content":"const message = \\"safe\\";"}'
```

### Execute in shadow

```sh
curl -X POST http://127.0.0.1:8080/api/sandbox \
  -H "content-type: application/json" \
  -d '{"command":"cargo test"}'
```

### Event stream

Connect a WebSocket client to `ws://127.0.0.1:8080/ws`. Events use the following shape:

```json
{
  "id": "74712b5a-4860-43e6-b0f7-f3836895699f",
  "timestamp": "2026-08-22T09:00:00Z",
  "kind": "ast",
  "status": "passed",
  "message": "AST validation passed",
  "command": null,
  "path": "src/example.ts"
}
```

## MCP

AgentGuard exposes a native MCP SSE transport:

```text
GET  http://127.0.0.1:8080/mcp/sse
POST http://127.0.0.1:8080/mcp/messages?sessionId=<session-id>
```

The SSE stream first sends an `endpoint` event containing the message URL. JSON-RPC responses are then delivered as `message` events. Supported MCP methods are `initialize`, `tools/list`, and `tools/call`.

Available tools:

- `agentguard_validate_ast` with `path` and `code`
- `agentguard_scan_secrets` with `content`
- `agentguard_sandbox_run` with `command`

The secret scanner honors `strictSecretScanning`: when enabled, detected secrets make the MCP tool result an error; when disabled, the result is a warning and the call can continue.

## Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix frontend run build
```
