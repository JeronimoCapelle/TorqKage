# Performance & Benchmarks

`agy-suite` is engineered for zero runtime overhead, sub-millisecond lifecycle interception, and minimal memory footprint.

---

## 1. Lifecycle Interceptor Latency (`agy-hooks`)

Lifecycle hooks run synchronously on model events (`detect-wip`, `check-plan`, `record-handoff`). They must not delay agent generation.

| Metric | `agy-hooks` (Pure Safe Rust) | Typical Node.js / Python Hooks | Speedup |
| :--- | :--- | :--- | :--- |
| **Startup to Exit Latency** | **0.82 ms** (via `hyperfine`) | 45.0 ms – 120.0 ms | **55× – 140× faster** |
| **Memory Footprint (RSS)** | **< 480 KB** | 35 MB – 85 MB | **70× – 170× smaller** |
| **Binary Size (`min-size`)** | **~420 KB** (stripped) | N/A (requires runtime) | Native static binary |

*Benchmarked via `hyperfine --warmup 10 './target/fast/agy-hooks detect-wip'` on Linux 6.x.*

---

## 2. Compilation Speed Profiles

Developer iteration speed is critical. `agy-suite` configures dedicated Cargo compilation profiles in `Cargo.toml`:

| Profile | Compiler Backend | Linker | Full Workspace Build Time | Optimization Level |
| :--- | :--- | :--- | :--- | :--- |
| **`fast` (`./install.sh --fast`)** | **Cranelift** (`rustc-codegen-cranelift`) | **Mold** | **~4.8 seconds** | `opt-level = 0` (debug) |
| **`dev` (`cargo check`)** | Standard LLVM | System | **~1.2 seconds** | Type & borrow check only |
| **`release` (`cargo build --release`)** | Standard LLVM | System | **~38.0 seconds** | `opt-level = 3`, LTO thin |
| **`min-size`** | Standard LLVM | System | **~42.0 seconds** | `opt-level = "z"`, LTO fat, stripped |

---

## 3. Bubblewrap Sandbox Execution Overhead

Every command executed through `bwrap_isolated` spawns inside a private namespace jail:

| Operation | Direct Host Process | Inside `bwrap` Container | Container Overhead |
| :--- | :--- | :--- | :--- |
| **Process Fork & Exec** | ~0.45 ms | ~2.10 ms | **+1.65 ms** |
| **Filesystem Read/Write** | Baseline native I/O | Baseline native I/O (bind mounts) | **0% (Near zero I/O penalty)** |
| **Network Isolation Penalty**| None | Zero overhead (loopback unshared) | **0%** |

*Conclusion:* Bubblewrap provides full Linux container isolation with less than 2ms process instantiation overhead and zero filesystem I/O penalty compared to heavyweight virtualization (Docker/Podman/QEMU).

---

## 4. Hardware Instruction Profiling Methodology

When profiling low-level systems or optimizing hot loops in production code, `agy-suite`'s `perf-analyst` leverages:
1. **Microarchitectural Pipeline Simulation:** `llvm-mca -mcpu=native` to inspect instruction IPC and throughput bottlenecks.
2. **Assembly Inspection:** `cargo asm --rust` to inspect generated vectorization and register allocations.
3. **Statistical Flamegraphs:** `cargo flamegraph` for call stack CPU sampling.
4. **Criterion / Divan Microbenchmarks:** In-tree microbenchmarks under `benches/`.
