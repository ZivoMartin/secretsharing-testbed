# Distributed Secret Sharing Protocol Tester

This project provides a framework for **testing and benchmarking distributed secret sharing protocols** in a realistic multi-machine environment. 

## Overview

The system is composed of three main components:

- **interface/** – the central orchestrator: reads a configuration file, connects to remote machines, launches nodes, and drives protocol operations.
- **manager/** – a daemon process to be run on each target machine. It receives instructions from the interface and launches or kills nodes accordingly.
- **nodes/** – the actual implementation of the nodes participating in the protocol. This includes cryptographic primitives and multiple protocol implementations. Each node is a separated process.

At the root level, shared data structures and message types are defined to facilitate communication between all components. A script is also provided to automate multi-machine deployment and execution.

---

## Quickstart

### Requirements

- Rust (>= 1.86)
- `gnuplot` (for plotting results)
- Linux (tested on Ubuntu 22.04)
- Passwordless SSH access to all remote machines

### Running an experiment (simplest case)

```bash
./run_experiment.py config/example.json machines.txt "172.81.22.10"
```

Where:

- `configs/example.json` is a configuration file (see configs/README.md)
- `machines.txt` contains one IP per line
- `"172.81.22.10"` is the IP of the interface's host machine; this machine may or may not also host a manager

---

## Project Structure

```
.
├── interface/         # Entry point; coordinates the whole experiment
├── manager/           # Daemon running on each remote machine
├── nodes/             # Contains protocol and crypto implementations
├── configs/           # All JSON configuration files
├── run_experiment.py  # Script to launch experiments from a single machine
├── shared/            # Common types for inter-component communication
└── README.md          # This file
```
---

## Results and Plots

After each run, the interface will:

- Collect timing data and logs from nodes
- Produce plots showing performance metrics 

The plots will be available in a `config/results/` directory of the interface's machine host
