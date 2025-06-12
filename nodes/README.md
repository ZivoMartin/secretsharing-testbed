# nodes/

This crate implements the logic of the **individual nodes** participating in distributed secret sharing protocols. Each node runs as a separate process and is capable of handling **multiple concurrent protocol instances**.

---

## Purpose

- Execute cryptographic protocols for secret sharing and reconstruction.
- Communicate with the interface to receive instructions and report status.
- Handle multiple concurrent protocol sessions, each identified independently.

---

## Architecture Overview

Each node is designed to support **several concurrent protocol instances**. Incoming messages from the interface contain a namespace or operation ID which is used to route the message to the correct handler.

This is managed by the central control component in `system/node_heart.rs`, referred to as the **node heart**.

The node heart is responsible for:

- Creating and managing multiple protocol instances
- Dispatching messages based on namespaces
- Collecting and forwarding outputs to the interface
- Handling lifecycles and clean shutdowns of operations

---

## File and Module Structure

```
src/
├── main.rs              # Entry point: receives messages and dispatches to the node heart
├── lib.rs               # Common exports
├── settings.rs          # Constants and global settings
├── macros.rs            # Procedural and helper macros
├── node/                # Core node behavior and abstractions
├── system/              # System-level coordination (node_heart, logging, etc.)
│   └── node_heart.rs    # Central operation manager on the node
├── crypto/              # Cryptographic primitives (BLS, etc.)
├── secure_message_dist/ # Secure message delivery system
├── [protocols]/         # One folder per protocol (see below)
```

---

## Entry Point

- `main.rs` initializes the node and connects to the interface.
- It forwards all messages to the **node heart**.
- The node heart uses the id and namespace in each message to route it to the appropriate protocol instance.

---

## Adding a New Protocol

To add a new protocol:

1. Create a new subdirectory (e.g., `my_protocol/`) in `src/`.
2. Implement the protocol logic using async functions and message passing.
3. Register the protocol in the protocol dispatcher, so the node heart can instantiate it.
4. Ensure messages are namespaced to distinguish concurrent runs.

---

## Cryptography

Cryptographic primitives are implemented or wrapped inside `crypto/` 
