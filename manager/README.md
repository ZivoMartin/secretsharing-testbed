# manager/

This crate implements the **remote controller daemon** that must be launched on each machine participating in the experiment.

It listens for TCP connections from the central `interface`, accepts incoming instructions, and is responsible for spawning or terminating local `node` processes.

---

## 🧭 Responsibilities

- Waits for instructions from the `interface`
- Can launch or terminate `node` processes
- Tracks which local processes are active
- Can reset and relaunch processes on demand
- Optionally logs CPU usage statistics (planned)

---

## ⚙️ Behavior Summary

1. **Startup**: Binds to a local IP (based on `MANAGER_IPS`) and starts listening for commands from the interface.
2. **Command Handling**:
   - `Generate(n)` spawns `n` new node processes with the right arguments.
   - `Connect(id)` registers a new node's ID to track it for later termination.
3. **Node Process Execution**:
   - Spawns `../target/release/nodes <interface_ip> <machine_ip>` as a subprocess
   - Uses `tokio::Command` to manage child process creation
4. **Process Reset**:
   - When new nodes are to be launched, any previously tracked nodes are killed via `kill -9`.

---

## ⚡ CPU Usage Logger

A background task is initialized to periodically poll CPU usage using the `sysinfo` crate.

---

## 🧱 File Structure

```
manager/
├── src/
│   └── main.rs         # Entire logic of the manager daemon
├── Cargo.toml
└── README.md           # This file
```

---

---

## ▶️ Running a manager

Managers are typically launched automatically by the top-level script (`run_experiment.py`), which connects via SSH and starts them on each machine.

However, you can also run a manager manually for local testing:

```bash
cargo run --release
```

Each manager must be running and accessible over TCP when the `interface` initiates the experiment.

------

## 💡 Notes

- Make sure `../target/release/nodes` is built before launching the interface.
- The interface assumes all managers are already running.
- You can simulate a cluster locally by running multiple managers on different ports.
