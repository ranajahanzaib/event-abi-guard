# CI integration

Commit a reviewed ABI snapshot in `.eventabi/baseline/Payments.json`. Place a version-1 manifest in `.eventabi/manifest.yaml` with `old: baseline/Payments.json`, `new: ../out/Payments.sol/Payments.json`, and your real consumer declarations. These paths resolve beside the manifest. The repository includes a runnable equivalent in `examples/foundry`.

The following workflow is for a Foundry repository using this tool. It installs version 0.1.0 from this repository's release commit (replace `main` with a reviewed commit SHA for reproducible CI). It does not assume a crates.io publication.

```yaml
name: Event compatibility
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: read
jobs:
  events:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: foundry-rs/foundry-toolchain@v1
        with:
          version: stable
      - name: Install EventABI Guard
        run: cargo install --git https://github.com/ranajahanzaib/event-abi-guard --branch main --locked eventabi-guard
      - name: Compile contracts
        run: forge build
      - name: Check event consumers
        shell: bash
        run: |
          status=0
          eventabi check --manifest .eventabi/manifest.yaml \
            --json-out eventabi-report.json \
            --markdown-out eventabi-report.md \
            --github-annotations || status=$?
          if test -f eventabi-report.md; then
            cat eventabi-report.md >> "$GITHUB_STEP_SUMMARY"
          fi
          exit "$status"
      - name: Upload reports
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: eventabi-report
          path: |
            eventabi-report.json
            eventabi-report.md
          if-no-files-found: ignore
```

In this tool's own repository, build with `cargo build --release --locked` and invoke `./target/release/eventabi` instead of installing from Git. For the included Foundry example, use `forge build --root examples/foundry` and `--manifest examples/foundry/.eventabi/manifest.yaml`. If your new ABI is committed JSON, omit Foundry entirely.

Reports are written before policy exit. The shell captures the CLI status, writes the summary, and exits with that status; `if: always()` preserves artifact uploads without masking a failing check. An input error may produce no report and still exits 2. No step depends on Ethereum infrastructure.

## Exit policy

- 0: valid analysis and no configured policy violation.
- 1: policy violation (breaking by default).
- 2: configuration, input, reporting, or execution error.

`--fail-on warning` also rejects structural warnings, including unused removals and uncertain relationships. `--fail-on never` is useful while reviewing migrations and still rejects invalid inputs. JSON stdout remains machine-readable; workflow commands are emitted to stderr, which GitHub Actions processes too. Annotation messages escape `%`, carriage returns, and newlines; metadata also escapes colons and commas. No source line coordinates are fabricated.

## Intentionally approving a change

First inspect the report and identify every consumer needing migration. Preserve the old event during a transition when practical. Coordinate consumer upgrades, update their declared expectations, and replace the committed baseline with a reviewed ABI snapshot only after the migration is accepted. Commit baseline/manifest changes together with the migration rationale in review. Updating the baseline is an explicit policy decision; the CLI never automatically accepts a breaking change.

Baseline approval does not prove runtime compatibility or successful indexer replay. Test actual consumers separately. Do not silence a real failure with a blanket `|| true` in a deployment gate.
