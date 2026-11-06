# EventABI Guard

Protect off-chain consumers from breaking smart contract event changes.

[![CI](https://github.com/ranajahanzaib/event-abi-guard/actions/workflows/ci.yml/badge.svg)](https://github.com/ranajahanzaib/event-abi-guard/actions/workflows/ci.yml) [![MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Contract upgrades can preserve every function while breaking indexers, billing services, and event subscribers. EventABI Guard compares two event ABIs against the expectations you declare for each consumer. It works offline: no RPC, wallets, compiler, or EVM required for existing ABI JSON.

```solidity
// Before
 event PaymentReceived(address indexed payer, uint256 amount);
// After: a different event signature and topic0
 event PaymentReceived(address indexed payer, uint256 amount, bytes32 referenceId);
```

```text
EventABI Guard
Contracts: 1 | Events examined (both snapshots): 2 | Consumers: 1
Breaking: 1 | Warning: 4 | Info: 1

[EAG001] Breaking EventRemoved (Definite)
Contract: basic | Consumer: billing-indexer
Event: PaymentReceived(address,uint256) -> none
...
Required baseline signature is absent. Existing topic filters and ABI
 decoders cannot consume it under this proposed ABI.
```

The warning findings describe structural changes and a possible same-name replacement. The breaking finding identifies the actual declared dependency that is no longer satisfied.

## Features

- Raw ABI arrays and Foundry/compiler artifacts containing `abi`.
- Canonical Solidity event signatures, Ethereum Keccak-256, tuples, arrays, overloads, anonymous events.
- Indexed layout changes detected even when topic0 stays the same.
- Consumer-specific checks for decoded logs and topic-only subscriptions.
- Named and zero-based positional field expectations.
- Strict baseline validation, deterministic findings and stable IDs.
- Human, versioned JSON, Markdown, and escaped GitHub annotations.
- Local analysis with explicit exit codes for CI.

## Installation

Install current stable Rust, then from a checkout:

```bash
cargo install --path . --locked
# Or build without installing:
cargo build --release --locked
./target/release/eventabi --help
```

The binary is `eventabi`; the reusable library is `eventabi_guard`. Building initially requires the dependencies in Cargo.lock; analysis requires no network access. This project is not yet advertised as published on crates.io.

## Quick start

All files below are committed in this repository:

```bash
# Structural differences alone: default exit 0, warnings describe potential risk.
cargo run -- diff --old examples/basic/old.json --new examples/basic/new.json

# Declared consumer break: expected exit 1.
cargo run -- check --manifest examples/basic/manifest.yaml

# Compatible addition: expected exit 0.
cargo run -- check --manifest examples/compatible/manifest.yaml

# Validate files and baseline expectations; accepts a breaking proposal.
cargo run -- validate --manifest examples/basic/manifest.yaml
```

## ABI inputs

`--old` and `--new` each accept a raw JSON ABI array or an artifact object with an `abi` array. Additional compiler metadata is ignored. Functions, constructors, custom errors, receive, and fallback entries do not participate in comparison. Missing or malformed ABI structures are errors, not empty contracts. Event `name`, `inputs`, `anonymous`, and each input's `name`, `type`, and `indexed` are required. Tuple components require `name` and `type`.

```bash
eventabi diff --old .eventabi/baseline/Payments.json --new out/Payments.sol/Payments.json
```

Foundry is optional; see the [Foundry example](examples/foundry/README.md).

## Consumer manifest

```yaml
version: 1
contracts:
  - id: Payments
    old: old.json
    new: new.json
consumers:
  - id: billing-indexer
    contract: Payments
    events:
      - signature: PaymentReceived(address,uint256)
        mode: decoded
        fields:
          - name: payer
            type: address
            indexed: true
          - position: 1
            type: uint256
  - id: payment-monitor
    contract: Payments
    events:
      - signature: PaymentReceived(address,uint256)
        mode: topic_only
```

Paths resolve relative to the manifest, regardless of your working directory. Signatures and field types use canonical ABI types (`uint256`, not `uint`). `decoded` checks event availability, anonymous status, whole-event indexed layout, and declared fields. `topic_only` checks signature-topic availability, disallows fields, and requires a non-anonymous baseline. A name binds by name; a position binds by position. If both are given, both must remain true. See the [manifest reference](docs/manifest.md).

## Compatibility classifications

| Severity | Meaning                                                                                    |
| -------- | ------------------------------------------------------------------------------------------ |
| breaking | A declared consumer expectation is violated.                                               |
| warning  | Structural risk or a possible same-name relationship, without proof of a consumer failure. |
| info     | An added event.                                                                            |

Unconsumed removals produce structural warnings, but no consumer violations. Name changes do not change topic0. Indexed changes do not change topic0 either, but they can break a decoded consumer even when its listed fields do not include the modified input.

Exact signatures establish event identity. Missing signatures remain definite removals; a sole unmatched same-name event is only a **possible** replacement. Ambiguous overloads are never paired. [Rules and certainty](docs/compatibility-rules.md) describe the distinctions.

## CLI reference

```text
eventabi diff --old PATH --new PATH [OUTPUT OPTIONS]
eventabi check --manifest PATH [OUTPUT OPTIONS]
eventabi validate --manifest PATH

OUTPUT OPTIONS:
  --format human|json|markdown       default: human
  --json-out PATH                    save a JSON report
  --markdown-out PATH                save a Markdown report
  --github-annotations              emit workflow commands to stderr
  --fail-on breaking|warning|never    default: breaking
```

`warning` fails on warnings or breaking findings. `never` permits all findings but never suppresses input or execution errors. Exit codes: **0** success under policy, **1** policy violation, **2** invalid input/configuration/execution. Reports contain no timestamps or absolute input paths. JSON stdout stays clean with annotations enabled. Output is uncolored, including when redirected or `NO_COLOR` is set.

```bash
eventabi check --manifest examples/indexed-change/manifest.yaml \
  --format json --json-out report.json --markdown-out report.md --github-annotations
```

The [JSON schema documentation](docs/report-schema.md) explains fields and ID stability.

## GitHub Actions

The complete [CI integration guide](docs/ci-integration.md) includes a copyable workflow that compiles a Foundry contract, checks a committed baseline, saves reports, and preserves a failing exit status. No separate JavaScript action is needed.

## Examples

| Directory                 | Change                                                                     | Expected check exit   |
| ------------------------- | -------------------------------------------------------------------------- | --------------------- |
| `examples/basic`          | Signature gains a parameter                                                | 1                     |
| `examples/indexed-change` | Indexed layout changes; only decoder fails                                 | 1                     |
| `examples/multi-consumer` | `total` renamed; named consumers fail, positional and topic consumers pass | 1                     |
| `examples/compatible`     | Unrelated event added                                                      | 0                     |
| `examples/foundry`        | Compile Solidity, compare with committed baseline                          | 1 after `forge build` |

Run `eventabi check --manifest examples/<directory>/manifest.yaml` for the JSON examples.

## Limitations and roadmap

ABI checks cannot prove runtime emission, event frequency or order, semantic meaning of values, successful historical replay, or that consumer code follows its manifest. Repeated identical types can hide semantic reordering. Fields currently bind only top-level inputs; a named tuple field also depends on its nested component names. See [limitations](docs/limitations.md).

Future work: Git-ref baselines, consumer source discovery, subgraph integrations, SARIF, deprecation policies, acknowledgment workflows, and cross-repository dependency tracking. These are not implemented MVP features.

## Development and contributing

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

See [architecture](docs/architecture.md), [contributing](CONTRIBUTING.md), and the [code of conduct](CODE_OF_CONDUCT.md). The test suite uses real CLI invocations and local fixtures; it needs no Ethereum infrastructure.

## License

[MIT License](LICENSE).
