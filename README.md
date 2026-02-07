# risbin

Fast “probably text / probably binary” classifier.

## CLI

```text
risbin [opts] --files-from <-|filelistfile>
risbin [opts] --files-from0 <-|file0listfile>
risbin [opts] <file> [file...]
```

Emits one record per path:

```text
{0|1|rc_shifted}{fs}{path}{rs}...
```

- `0` = probably text
- `1` = probably binary
- `rc_shifted` = per-file error code (only when `--error-flag +1`)

Key options:

- `--abs` output absolute paths (formatting only)
- `--fs <str>` / `--rs <str>` literal separators (defaults: TAB / LF)
- `--fs0` / `--rs0` NUL separators
- `--error-flag <+1|pass|warn|stop>` (default: `warn`)
- `-g/--glob`, `-G`, `-i/--iglob`, `-I` (ripgrep-style globs)

## Library

Library users can avoid pulling in CLI-only deps:

```toml
risbin = { version = "0.1.0", default-features = false }
```

API entry points:

- `risbin::Classifier`
- `risbin::classify_bytes`
- `risbin::is_binary_path`

## Behavior notes

- Binary heuristic is ripgrep-like: stream-read and treat any NUL byte as binary.
- Like ripgrep’s default encoding behavior, inputs are BOM-sniffed/transcoded first, so UTF-16-with-BOM is treated as text.

## License

Dual-licensed under the UNLICENSE or MIT, at your option (same as ripgrep).
See `UNLICENSE`, `LICENSE-MIT`, and `COPYING`.

