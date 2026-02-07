# Upstream ripgrep test sources (vendored for reference)

These files are copied from the local `..\ripgrep` checkout so `risbin` can keep behavior aligned with ripgrep’s well-tested binary/encoding handling.

They are **not** compiled as part of `risbin`’s test suite (they live under `tests/upstream/...`), but serve as:

- a reference for “what ripgrep asserts”
- a convenient place to pull more cases from as `risbin` grows

Source:

- repo: `..\ripgrep`
- revision: `0a88cccd5188074de96f54a4b6b44a63971ac157`
- files copied:
  - `tests/binary.rs`
  - `tests/feature.rs` (contains UTF-16 BOM related tests)
  - `tests/data/sherlock-nul.txt`

`risbin`’s adapted/active test coverage for these behaviors currently lives in:

- `tests/upstream_ripgrep.rs`
- `tests/data/sherlock-nul.txt`

