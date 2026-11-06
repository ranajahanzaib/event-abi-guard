# Manifest version 1

A manifest has required `version: 1`, `contracts: [...]`, and `consumers: [...]`. Empty lists are permitted. Every mapping rejects unknown fields. YAML duplicate keys are rejected. Files are local UTF-8, bounded to 16 MiB; no URL fetching, environment expansion, includes, or commands are configured by the tool.

A contract contains unique nonempty `id`, `old`, and `new` file paths. Both snapshots must load successfully, including for `validate`. Relative paths resolve beside the manifest; absolute paths are accepted. Compiler artifacts may contain arbitrary unrelated metadata.

A consumer has unique nonempty `id`, optional `description`, a known `contract` ID, and `events`. Each event declaration has a canonical `signature`, required `mode`, and optional `fields` (default empty). Duplicate signatures within a consumer are errors. Overloads must be declared with their full signatures; names alone never resolve an event.

```yaml
version: 1
contracts:
  - id: Payments
    old: baseline/Payments.json
    new: ../out/Payments.sol/Payments.json
consumers:
  - id: billing
    description: Indexes payments
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
```

## Modes

`decoded` assumes the consumer uses the complete baseline event ABI to decode logs. Missing signature, changed anonymous status, or any indexed flag change breaks the dependency, even with an empty field list. Listed fields add application expectations; they do not reduce the decoder's binary layout requirements.

`topic_only` subscribes solely using topic0. Its baseline must be non-anonymous and its fields must be empty. Indexed modifiers and names do not affect that signature subscription. Missing signature or conversion to anonymous breaks it. This does not guarantee the event is actually emitted at runtime.

## Fields

Every field requires nonempty `name`, zero-based integer `position`, or both. A name must resolve uniquely in the baseline; empty or duplicated input names cannot be guessed. A position must exist. `type` and `indexed` are optional assertions that must agree with the baseline. Use canonical types such as `uint256`, `(address,uint256)[]`, or `bytes32[2]`. Aliases in ABI files normalize, but manifest expectations must already be canonical.

- Name alone: the decoded property must still exist with the baseline type and position. A rename fails even when topic0 is identical.
- Position alone: the positional value is required; its label is informational. Name-only changes pass.
- Both: the field must retain both its position and name.
- A named tuple field includes the decoded property names of its tuple components, recursively. Positional tuple fields do not depend on component labels. Individual nested field selectors are deferred.

Duplicate field expectations resolving to the same baseline input are rejected, even if identical. This makes contradictory name/position declarations explicit. Missing events, absent/ambiguous fields, incorrect types, and incorrect indexed expectations are baseline configuration errors (exit 2), never new ABI regressions (exit 1).

`validate` verifies the complete configuration and both snapshots without enforcing compatibility policy. `check` performs the same validation before emitting findings.
