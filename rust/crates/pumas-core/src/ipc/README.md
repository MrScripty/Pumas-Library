# IPC Module

Local TCP IPC for transparent instance convergence between pumas-core hosts.

## Purpose

When multiple host applications need the same library, only the first instance
becomes the **Primary** (running all subsystems locally). Subsequent instances
become **Clients** that connect via TCP and proxy calls transparently. This
avoids resource contention and ensures consistent state.

IPC startup is coordinated by the registry claim flow:

- a primary contender first writes a `claiming` row in the registry
- the IPC server binds a local port
- the claim row is promoted to `ready` with that port
- constructors and wrapper layers attach only to ready rows and wait while startup is still claiming

## Protocol

- **Transport**: TCP on `127.0.0.1:0` (OS-assigned port, stored in registry)
- **Framing**: 4-byte big-endian length prefix + UTF-8 JSON payload
- **Messages**: JSON-RPC 2.0 format

```
Client -> Server: [u32 BE: len][{"jsonrpc":"2.0","method":"ping","params":{},"id":1}]
Server -> Client: [u32 BE: len][{"jsonrpc":"2.0","result":"pong","id":1}]
```

Maximum frame size: 16 MiB (configurable via `RegistryConfig::MAX_IPC_MESSAGE_SIZE`).

## Files

- `mod.rs` - Module exports
- `protocol.rs` - Frame read/write functions and JSON-RPC type definitions
- `server.rs` - TCP server with `IpcDispatch` trait for method routing
- `client.rs` - TCP client with `call()` method for transparent proxying

## Thread Safety

- **Server**: Runs on the tokio runtime. Each connection is handled in a tracked
  spawned task, bounded by `MAX_IPC_CONNECTIONS`; dropping the server handle
  aborts any remaining connection tasks after broadcasting shutdown.
- **Client**: Uses `tokio::Mutex` to serialize access to the TCP stream,
  allowing safe concurrent use from multiple async tasks.

## Error Handling

When a Client detects a broken TCP connection (server crashed, network error),
it returns `PumasError::SharedInstanceLost { pid, port }` so the host app
can decide to reconnect or create a new Primary instance.

Default startup paths do not start a second primary when they lose the race.
They wait for the winning claim to become `ready`, then attach as clients over
IPC. If the winning process never reaches readiness, startup returns
`PumasError::PrimaryInstanceStartupTimeout`.
