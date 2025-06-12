# interface/

This is the **main orchestrator** of the distributed secret sharing protocol testing framework. It is the primary entry point of the system and is responsible for:

- Parsing the JSON configuration file
- Establishing TCP connections to all remote managers
- Instructing each manager to spawn the required number of nodes
- Driving protocol execution step-by-step across all nodes
- Collecting performance metrics and generating plots

---

## ⚙️ Usage

This component is not meant to be launched directly. Instead, it is executed by the top-level script `run_experiment.py`.

However, you may manually run it for debugging purposes:

```bash
cargo run --release -- <path_to_config.json> <path_to_machine_list>
```

---

## 📂 Directory Structure

```
interface/
├── src/
│   ├── main.rs             # Entry point
│   ├── network.rs          # Handles all network primitives of the interface
│   ├── process.rs          # Represents a secret sharing operation, started by the interface and run in parallel
│   ├── configuration.rs    # Struct representing a configuration
│   ├── managers_getter.rs  # Extracts managers' IPs from the IP file
│   └── base_generator.rs   # Generates two BLS12-381 points
├── Cargo.toml
└── README.md               # This file
```

---

## 📞 Communication Protocol

The interface communicates with each manager using a lightweight custom protocol over TCP. Messages are serialized using `serde`. The shared types are defined in the root shared library.

---

## ✏️ Notes

- Interface assumes managers are already running on the machines listed in the IP file.
- TCP errors and node failures are logged and handled gracefully whenever possible.
