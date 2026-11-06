# JSON report schema 1

`--format json` writes a single valid JSON document to stdout; diagnostics and optional workflow commands use stderr. `--json-out PATH` writes the same document plus a newline. No timestamps, source locations, or host-specific paths are included.

Top-level fields:

- `schema_version`: integer `1`.
- `tool`: `name: "eventabi"`, `version`: package version.
- `summary`: integer `contracts`, `consumers`, `events_examined`, `breaking`, `warnings`, `info`. Events examined is the sum of event counts in both snapshots, not unique signatures.
- `findings`: deterministically sorted array, possibly empty.

Each finding has:

| Field                                | Type / meaning                                                 |
| ------------------------------------ | -------------------------------------------------------------- |
| `id`                                 | Full lowercase `0x` Keccak-256 of structured finding identity. |
| `rule`                               | Stable EAG rule ID.                                            |
| `contract`                           | Contract identifier, or `contract` for standalone diff.        |
| `consumer`                           | Consumer ID or null for structural findings.                   |
| `severity`                           | `breaking`, `warning`, `info`.                                 |
| `category`                           | Uppercase structural category.                                 |
| `certainty`                          | `definite` or `possible`.                                      |
| `previous_event`, `proposed_event`   | Canonical signatures or null.                                  |
| `previous_topic0`, `proposed_topic0` | Emitted signature topics or null (including anonymous events). |
| `field`                              | Name or zero-based positional component path, or null.         |
| `previous_value`, `proposed_value`   | Relevant string values or null when unavailable.               |
| `explanation`, `remediation`         | Human-readable impact and suggested next action.               |

IDs hash a JSON tuple of contract, consumer, rule, category, signatures, field, and previous/proposed values. Prose, tool version, paths, timestamps, and array order are excluded. Equivalent findings have identical IDs. A consumer rename intentionally changes its finding identity. Structural removal/addition findings need not contain values because their signatures already convey the change.

Sort order: contract, previous signature, proposed signature, consumer, rule, field, ID. Nulls sort before strings. Formatter counts all come from the same finalized report. Treat explanation/remediation text as display text; use stable IDs, categories, severities and signatures for automation. A future incompatible field/type change requires a new schema version.
