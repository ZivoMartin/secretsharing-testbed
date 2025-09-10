# Configuration Format for Experiments

This directory contains all the configuration files used to drive experiments for testing secret sharing protocols.

Each configuration file is a JSON array of experiment blocks. The format is designed to be flexible and expressive to support a wide range of benchmarking scenarios.

---

## Global Block

The first object in the array is a **global configuration block**, specifying how results are handled:

```json
{
  "output": "output",
  "result_type": ["average", "median", "details"]
}
```

- `output`: directory where all experiment results will be saved.
- `result_type`: list of output formats:
  - `"average"`: compute the mean over all trials
  - `"median"`: compute the median
  - `"details"`: store raw results for each run

---

## Experiment Blocks

Each subsequent object in the array is an **experiment definition**. Two types are supported:

### 🔹 Latency Test

Measures how long it takes to complete secret sharing operations.

```json
{
  "latency": {
    "hmt": 15
  },
  "output_file": "res_lat",
  "setup": {
    "algos": ["avss_simpl", "bingo"],
    "n": [10, 30],
    "batch_size": 10,
    "dealer_corruption": 0
  }
}
```

- `hmt`: number of repetitions per setup (to compute mean/median).
- `output_file`: filename (inside `output/`) to store results.
- `setup`: describes which parameters to combine:
  - `algos`: list of protocols to test.
  - `n`: list of network sizes.
  - `batch_size`: size of the batch.
  - `dealer_corruption`: 0 (honest) or 1 (malicious dealer).

### Debit Test

Measures throughput over a fixed time window.

```json
{
  "debit": {
    "duration": 16
  },
  "output_file": "res_debit",
  "setup": {
    "algos": ["avss_simpl", "bingo"],
    "n": [30],
    "batch_size": 10,
    "dealer_corruption": 0
  }
}
```

- `duration`: how long to run the test (in seconds).
- Other fields same as in latency.

---

## Automatic Combinations

Each experiment block will automatically test **all combinations** of the setup parameters:
- For example, with 2 algorithms and 2 network sizes, 4 configurations will be tested.

---

## Notes

- You may vary `n`, `batch_size`, or even `dealer_corruption` to evaluate influence of different parameters.
- All experiments share the global `result_type` setting from the first block.
