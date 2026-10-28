# dmn-viewer

View, edit, check, test and fix Camunda DMN decision tables, with no installer. A Windows desktop app and a command-line checker for CI, each a single `.exe`.

- **Edit in place.** Cells, headers, hit policy, rules and columns. Saving changes only the edited bytes; the diagram, namespaces, vendor attributes, comments and formatting stay exactly as they were.
- **Find problems.** Invalid FEEL, unreachable and overlapping rules, values outside the allowed list, placeholders, unused inputs, JavaScript portability and more.
- **Fix them one at a time.** Each automatic fix shows the change from → to and its effect on your tests before you confirm it. Every fix can be undone.
- **Test.** Evaluate the table for any inputs with a per-rule trace, keep test suites next to your DMN files, and see which rules your tests never reach.
- **Check in CI.** `dmn-check` reads files, folders and globs and writes text, JSON, JUnit XML or SARIF.

## Quick start

    cargo build --release
    target\release\dmn-viewer.exe examples\shipping-fees.dmn
    target\release\dmn-check.exe examples\shipping-fees.dmn

Requires Windows with the MSVC toolchain and Rust 1.92 or newer. The C runtime is linked statically, so the executables run on a clean machine.

<details>
<summary><b>Desktop app</b></summary>

    dmn-viewer.exe [file.dmn] [--tab rules|issues|test|xml] [--case N] [--rule N] [--dark]

- Double-click a cell, header or annotation to edit it. Right-click a rule or header to insert, duplicate, move or delete.
- Ctrl+S saves, Ctrl+Z / Ctrl+Y undo and redo, Ctrl+D reviews every unsaved change before saving.
- **Fix issues** goes through the issues that have an automatic fix: Enter applies, S skips, Esc stops. The table is read-only meanwhile and nothing is saved until you save.

</details>

<details>
<summary><b>Command-line checker</b></summary>

    dmn-check.exe <file.dmn | dir | glob> [...] [--test suite.json] [--coverage]
                  [--json | --junit | --sarif] [--fail-on error|warn|never]

Exit code 1 when an error is found or a test fails, 2 for a usage error. Quote globs such as `"rules/**/*.dmn"`. `--fail-on warn` also fails on warnings. SARIF works with GitHub code scanning.

</details>

<details>
<summary><b>Test suites</b></summary>

A `<name>.tests.json` next to `<name>.dmn` is loaded automatically:

```json
{
  "decision": "ShippingFee",
  "cases": [
    { "name": "EU small parcel", "inputs": { "Region": "EU", "Weight": 2.5 }, "expected": { "Fee": 4.5 }, "expectRules": [1] },
    { "name": "Missing date", "inputs": { "Region": "EU", "Weight": 30 }, "expectError": true }
  ]
}
```

</details>

<details>
<summary><b>Development</b></summary>

    cargo test
    cargo fmt --check
    cargo clippy --all-targets

| Path | Contents |
|---|---|
| `src/model.rs`, `src/parse.rs` | DMN model and XML parsing |
| `src/edit.rs`, `src/diff.rs` | Byte-preserving edits; unsaved changes as line hunks |
| `src/lint.rs`, `src/fix.rs` | The checks, and their automatic fixes |
| `src/unary.rs`, `src/eval.rs` | Unary tests; the FEEL, JavaScript and JUEL evaluator |
| `src/suite.rs`, `src/session.rs` | Test suites, rule coverage, and the tester's state |
| `src/report.rs`, `src/targets.rs` | `dmn-check` output formats; directory and glob expansion |
| `src/main.rs`, `src/document.rs`, `src/fixing.rs` | Desktop app, its open file, and its fix session |
| `src/bin/dmn-check.rs` | Command-line checker |
| `examples/`, `tests/`, `assets/` | Example table and suite, integration tests, embedded fonts and icons |

Real decision tables stay out of the repository: `.gitignore` excludes every `*.dmn` and `*.tests.json` outside `examples/`. [GLOSSARY.md](GLOSSARY.md) defines the domain terms used in the code.

</details>

## License

MIT, see [LICENSE](LICENSE). The bundled fonts and icons have their own licenses, listed in [assets/LICENSES.txt](assets/LICENSES.txt).
