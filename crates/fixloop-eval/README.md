# fixloop-eval

Evaluation harness for comparing models (frontier vs local) on the same known-bug cases.

- Cases live in [`eval/cases/`](../../eval/cases/), one YAML file each: a diff plus the bugs a good reviewer should report. A case with no expected bugs is a clean control: any finding is a false alarm.
- `score` matches findings to expected bugs (same path, line within ±3) and reports **found**, **false alarms** and **missed**. `summarize` gives precision, recall and fix pass rate.

Status: case format, loading and scoring are done (Phase 1). Running models over the cases needs the Reviewer (Phase 2). A scheduled CI job comparing models follows then.

License: Apache-2.0
