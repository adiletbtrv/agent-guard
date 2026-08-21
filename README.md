# AgentGuard

AgentGuard is a local firewall and reversible execution layer for autonomous coding agents. The Rust daemon validates TypeScript and JavaScript syntax with tree-sitter, scans proposed content for credentials, executes commands in an isolated shadow copy, and broadcasts decisions to the React control plane over WebSockets.

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

## Verification

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix frontend run build
```
