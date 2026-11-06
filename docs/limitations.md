# Limitations

This is ABI-based static compatibility analysis against explicitly declared dependencies, not a security audit, compiler, bytecode analysis, or formal guarantee of application behavior.

It cannot establish whether events are emitted, emission frequency or order, value semantics, business identity of same-typed parameters, historical replay success, whether consumers actually deploy their declared ABI, or whether consumer code follows the manifest. A semantic swap of two identically typed unnamed fields may be invisible. ABI JSON cannot prove a same-name event is a replacement rather than a new overload.

The analyzer retains definite removals/additions and labels possible same-name relationships. No cross-name heuristic matching is performed. `diff` alone does not prove consumer breakage; its warnings can be gated explicitly with `--fail-on warning`.

Supported types: address, bool, string, bytes, bytes1–32, uint/int widths 8–256 in multiples of 8, uint/int aliases, function, tuples, and fixed/dynamic arrays. Fixed/ufixed types and source-language types such as mappings, address payable, enums, user-defined value type names, and contract names must be compiled into supported ABI representations first. Unsupported grammar fails instead of producing a guessed hash.

Each event must explicitly provide standard JSON event properties. Recognized non-event ABI entries are ignored rather than fully validated. Duplicate canonical signatures are rejected, even if their input objects are identical. Duplicate/empty parameter names can be represented but cannot resolve a name-based field uniquely. No JSON source line mapping is attempted.

Field selectors address top-level inputs only. A named tuple selector includes its nested component names; positional selectors treat labels as informational. Fine-grained nested component dependencies and generated consumer source discovery are deferred.

File inputs are limited to 16 MiB each. JSON uses serde_json's default recursion limit; tuple normalization caps tuple depth at 24 and array dimensions at 32. Fixed dimensions must be positive and fit u64. YAML uses serde-saphyr's default parser/alias budgets and duplicate-key rejection. These limits deliberately reject excessively complex inputs.

The analyzer never executes manifest data, expands commands, or performs network calls. Output files are ordinary local writes and overwrite an existing destination; create destination directories beforehand. GitHub annotations carry no source coordinate because ABI artifacts do not provide trustworthy source mappings. Markdown is escaped for untrusted strings.

Foundry is optional and only needed for the Solidity compilation example. Installing/building Rust dependencies requires a populated cache or network access; the resulting binary runs offline. No RPC, storage-layout, function compatibility, EVM execution, automatic baseline acceptance, or remote Git-reference resolution is implemented.
