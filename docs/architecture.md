# Architecture

A single Rust package provides the `eventabi_guard` library and a small clap binary. No runtime, concurrency, database, or network client is involved.

1. `abi::load` reads a local file with a 16 MiB limit. `abi::parse` accepts a raw ABI or extracts an artifact's `abi`. serde_json rejects malformed JSON and bounds JSON nesting. Event properties are validated; other recognized ABI items are ignored. BTreeMap indexes events by canonical signature and rejects duplicates, including duplicates after alias normalization.
2. A small bounded type grammar normalizes `uint`/`int`, validates widths and array suffixes, and recursively expands tuple components. Tuple nesting is capped at 24, array dimensions at 32, fixed dimensions at positive u64 values. `sha3::Keccak256` hashes the canonical signature; anonymous events retain a signature but no emitted topic0. See the [Solidity ABI specification](https://docs.solidity.org/en/latest/abi-spec.html).
3. `diff::compare_abis` compares exact signature identities first. Missing events remain removals/additions. A unique unmatched same-name pair provides possible explanatory details without replacing those facts. Multiple unmatched overloads remain unpaired. Parameter and tuple properties are compared recursively. Indexed layout is always compared positionally, even when names suggest reordering.
4. `consumer::load` parses YAML with serde-saphyr, whose default budgets and duplicate-key rejection remain enabled. Unknown configuration fields are errors. Validation resolves paths relative to the manifest, loads both snapshots, validates unique identifiers, and checks every dependency against the old baseline. The validated wrapper cannot be constructed directly outside the module.
5. `consumer::check` first records structural findings, then evaluates dependencies. Missing required signatures fail definitively without requiring a replacement guess. Topic-only dependencies ignore indexed/name changes. Decoded dependencies check anonymous status, whole-event indexing and declared fields. Exact signatures imply unchanged canonical type sequences; if types change, missing event detection already rejects the old decoder. No heuristic candidate is used as a definite new event.
6. `report::Report` is the sole report source. Finalization computes counts, hashes length-delimited structured finding identities with Keccak-256, and sorts by contract, event, consumer, rule, field, and ID. Human, JSON, Markdown, and workflow-command formatters contain no compatibility logic. Workflow commands go to stderr so JSON stdout remains parseable.

## Dependency choices

clap handles CLI parsing; serde and serde_json handle JSON; serde-saphyr is the maintained YAML parser; sha3 supplies established Keccak; thiserror supplies typed library errors. The CLI propagates execution errors through a boxed standard error and returns code 2.

A general ABI encoding library such as alloy-json-abi was considered. This tool does not encode/decode values, so the MVP uses a small explicitly bounded type normalizer to preserve field/component metadata and reject unsupported grammar. The parser is covered by signature vectors and malformed-type tests; unsupported fixed-point types are documented rather than guessed.

## Library usage

```rust
use eventabi_guard::{abi, diff};
let old = abi::parse("[]")?;
let new = abi::parse("[]")?;
assert!(diff::compare_abis(&old, &new).is_empty());
# Ok::<(), eventabi_guard::Error>(())
```

Manifest users call `consumer::load(path)` followed by `consumer::check(&validated)`. Direct ABI diffing depends on neither YAML nor terminal formatting.
