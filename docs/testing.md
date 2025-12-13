# Testing

This document describes Wixer's testing strategy, with an emphasis on meaningful coverage for the YAML <-> XML authoring workflow.

## Goals

- Prefer fast, deterministic tests that validate core behavior.
- Focus first on correctness of YAML <-> XML conversion semantics.
- Keep production source files from being dominated by tests.

## Test organization

### In-memory semantic tests (preferred)

These tests avoid disk I/O and validate conversion semantics using in-memory APIs.

- YAML <-> XML: `wixer::yaml_to_wix::convert_value_to_string`
- XML <-> YAML: `wixer::wix_to_yaml::convert_xml_str_to_yaml_str`

These tests should assert semantic properties (attributes, child elements, text content, repeated elements) rather than relying on exact output formatting.

Examples of semantic test coverage:

- Attribute strategies: `Inline`, `Prefix`, `Separate`
- Text nodes via `_value` / `value` / `text`
- Repeated elements via YAML sequences
- Namespace declarations such as `xmlns` and `xmlns:*`
- Roundtrip semantics (YAML<->XML<->YAML) with normalized assertions

The bulk of these tests live in `wixer/tests/`.

### Minimal file-based integration tests

Some behaviors inherently require disk I/O and are covered by small integration tests that create temporary files and directories.

Examples:

- `yaml_to_wix::convert_file` rejecting XML content in a `.yaml` file
- End-to-end `!include` resolution:
  - mixins inside mappings
  - include wrappers inside sequences
  - sequence append/merge behavior
  - recursion/cycle error surfacing

These tests should be kept minimal and focused, and should not attempt full end-to-end installer builds.

### Shared test utilities

To avoid tests dominating production code, shared helpers should live under:

- `wixer/tests/support/`

## Running tests

- Workspace tests:
  - `cargo test --workspace`

- Single crate tests:
  - `cargo test -p wixer`

## Coverage workflow

Coverage is generated using `cargo llvm-cov` and can be displayed in VS Code using Coverage Gutters.

- Generate LCOV:
  - `cargo make coverage`
  - `cargo make coverage-refresh`

- Summarize LCOV:
  - `cargo make coverage-summary`
  - `cargo make coverage-summary-yaml-xml`

The `wixer/tools/coverage_summary.py` script is a reusable LCOV summarizer.

## Pre-commit quality gates

The repository uses `pre-commit` to enforce formatting and linting locally (matching CI expectations):

- `cargo fmt --all -- --check`
- `cargo clippy --workspace -- -D warnings`

Setup:

- `pre-commit install`
- Run all hooks:
  - `pre-commit run --all-files`
