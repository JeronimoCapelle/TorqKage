# The Verification Sandwich

High-assurance systems development in `agy-suite` follows the **Verification Sandwich** methodology across 4 distinct phases:

```
┌────────────────────────────────────────────────────────┐
│  Phase 1: Contract & Specification (Inception)        │
│  - Planner defines architectural boundaries            │
│  - Spec-Checker audits hardware specs and RFCs        │
└──────────────────────────┬─────────────────────────────┘
                           │
┌──────────────────────────┴─────────────────────────────┐
│  Phase 2: Implementation & DV Sandwich                 │
│  - Test-Author writes adversarial verification test    │
│  - Coder implements logic to satisfy test harness      │
│  - Doc-Author maintains API specs and rustdoc          │
└──────────────────────────┬─────────────────────────────┘
                           │
┌──────────────────────────┴─────────────────────────────┐
│  Phase 3: Optimization & Release Gatekeeping           │
│  - Perf-Analyst profiles instructions and benchmarks   │
│  - Finisher verifies zero mutants, deny, and clean git │
└──────────────────────────┬─────────────────────────────┘
                           │
┌──────────────────────────┴─────────────────────────────┐
│  Phase 4: Exact Computation (JIT Support)              │
│  - Mini solves exact symbolic math and bitfield logic  │
└────────────────────────────────────────────────────────┘
```

This prevents architectural drift, guarantees mutation resistance, and maintains zero regressions.
