# PHASE 1 — TODO (risbin)

Checklist for implementation work. Move items from **TODO** → **Done** as they land.

## TODO

## Done

- Create Rust package with `lib` + `bin` targets; default feature `cli` on; `[[bin]]` `required-features = ["cli"]`.
- Define public library API (`Classifier`, `classify_path`, `classify_reader`, `classify_bytes`, `is_binary_path`).
- Implement core binary heuristic (ripgrep strategy): stream-read + NUL scan (`memchr`), stop at first NUL.
- Implement BOM-sniffed decoding for `--files-from` and `--files-from0` (ripgrep-like via `encoding_rs_io`).
- Implement input source handling and exclusivity:
  - `--files-from` (LF-delimited; `-` = stdin)
  - `--files-from0` (NUL-delimited; `-` = stdin)
  - positional file paths
- Enforce “no directories” input rule (treat directory as per-file error).
- Implement glob filtering:
  - `-g` / `-i` pass-through to `ignore::overrides::OverrideBuilder`
  - `-G` / `-I` implicit-negation; reject patterns starting with unescaped `!`
- Implement output formatting:
  - `--abs` output-only (abspath-ish; `path-absolutize`)
  - `--fs <str>` / `--rs <str>` literal strings
  - `--fs0` / `--rs0` => NUL separators
- Implement `--error-flag <+1|pass|warn|stop>` + exit code policy.
- Add integration tests for CLI output bytes (including `--fs0`/`--rs0`) and file-list inputs.
- Vendor upstream ripgrep test sources + fixtures for reference/porting (`tests/upstream/ripgrep/...`).
- Add a small “vendored from ripgrep” tracking note (`tests/upstream/ripgrep/README.md`).
- Add `README.md` with usage and “as a library” snippet (`default-features = false`).
