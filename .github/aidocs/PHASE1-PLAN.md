# PHASE 1 - risbin (Rust CLI) plan

Goal: a fast CLI that classifies each input path as **probably text (0)** or **probably binary (1)** (or reports an error code when configured) and prints one record per path:

```text
{0|1|rc_shifted}{fs}{path}{rs}{0|1|rc_shifted}{fs}{path}{rs}...
```

## 1) Reference: ripgrep's binary heuristic

Ripgrep's "binary file detection" is intentionally simple: **if a NUL byte (`0x00`) is seen while reading, the file is treated as binary**.

Primary implementation points in the local `..\ripgrep` checkout:

- `..\ripgrep\crates\searcher\src\line_buffer.rs`: `BinaryDetection::{Quit,Convert}` scans newly-read bytes for a configured "binary byte" (ripgrep typically uses NUL) and records `binary_byte_offset` at the first occurrence.
- `..\ripgrep\crates\core\flags\defs.rs`: docs for `--binary` / `--text` describe the NUL-based heuristic and how search behavior changes.

For `risbin`, we can adopt the same core heuristic for Phase 1: **binary if any NUL byte is observed while reading** (see §3.5).

## 2) Phase 1 deliverables

- `risbin` Rust workspace with a single binary crate (`risbin`).
- CLI parsing covering:
  - Input sources:
    - `risbin [opts] --files-from <-|filelistfile>`
    - `risbin [opts] --files-from0 <-|file0listfile>`
    - `risbin [opts] <file> [file...]`
    - All paths from these sources are treated as file paths (non-directories); directories are errors.
  - Output formatting:
    - `--abs` (output only; does not affect matching/traversal)
    - `--fs <str>` field separator (default tab). No escape language.
    - `--rs <str>` record separator (default LF). No escape language.
    - `--fs0` set field separator to NUL (`\0`)
    - `--rs0` set record separator to NUL (`\0`)
    - `--error-flag <+1|pass|warn|stop>` (default: `warn`)
  - Filtering (ripgrep-style globs):
    - `-g <glob>` (like ripgrep `-g/--glob`)
    - `-G <glob>` (like ripgrep `-g`, but every pattern is implicitly negated)
    - `-i <glob>` (like ripgrep `--iglob`)
    - `-I <glob>` (like ripgrep `--iglob`, but every pattern is implicitly negated)
- Streaming classification over inputs with stable ordering (input order == output order).
- Basic unit tests for:
  - separator parsing (`--fs`, `--rs`, `--fs0`, `--rs0`)
  - path formatting (`--abs`)
  - binary heuristic (NUL vs non-NUL)
  - glob filtering semantics (`-g/-G/-i/-I`)
  - `--error-flag` behavior

## 3) Design decisions (Phase 1)

### 3.1 CLI parsing library

Prefer `lexopt` (small, fast, explicit). If you want full `--help` generation and more ergonomics, use `clap`.

### 3.2 Input enumeration

Inputs become a stream of `PathBuf` from exactly one of:

1. `--files-from` (newline-delimited; `-` means stdin)
2. `--files-from0` (NUL-delimited; `-` means stdin)
3. Positional args (`<file> [file...]`)

If both `--files-from*` and positional args are given, error.

No directory traversal in Phase 1: every input path must refer to a non-directory file. Any directory encountered should be treated as an input error (see §3.6).

### 3.3 Output path formatting

Default formatting (no `--abs`): **match ripgrep `--files`**: print paths as provided by the input stream (typically relative, no special rewriting).

`--abs`: **output formatting only**. Convert each output path to an absolute path in the sense of Python's `os.path.abspath` (join with CWD + normalize, but do not require existence and do not resolve symlinks like `canonicalize` does).

Implementation note: use a small "pathlib-ish" crate (e.g., `path-absolutize`) instead of `std::fs::canonicalize`.

### 3.4 Glob filtering (`-g/-G/-i/-I`)

Implement glob filtering by borrowing ripgrep's approach:

- Use `ignore::overrides::OverrideBuilder` rooted at `cwd`.
- `-g`: add the pattern as provided (supports ripgrep's leading `!` negate syntax).
- `-i`: add the pattern as provided, but force case-insensitive matching (ripgrep's `--iglob` behavior).
- `-G`: same as `-g`, but treat every provided pattern as negated by default (as if it had a leading `!`).
- `-I`: same as `-i`, but treat every provided pattern as negated by default.

Notes (matching ripgrep/ignore parsing):

- `!` is only special as the first **unescaped** character of the whole pattern; `\\!` makes it literal.
- To keep `-G/-I` semantics simple, patterns for `-G/-I` must not start with an unescaped `!` (usage error; use `-g/-i` instead).

### 3.5 Binary/text heuristic (ripgrep strategy)

Phase 1 heuristic:

- Stream-read the file in fixed-size chunks (reuse a buffer).
- If any `0x00` occurs: classify as `1` (probably binary) and stop reading.
- If EOF is reached with no `0x00`: classify as `0` (probably text).

This matches ripgrep's core behavior: binary detection is not a separate pre-pass; it happens while reading, and can "quit" at the first NUL. Internally, ripgrep's default line buffer capacity is 64 KiB, which is a reasonable chunk size here too.

Encoding note:

- Like ripgrep's default `--encoding auto`, `risbin` should BOM-sniff and transcode UTF-16/UTF-8-with-BOM before applying the NUL heuristic, so UTF-16 text with a BOM is treated as text.

### 3.6 Errors and exit behavior (`--error-flag`)

Input errors (missing file, permission denied, directory, broken symlink, etc.) are controlled by:

`--error-flag <+1|pass|warn|stop>` (default: `warn`)

Modes:

- `pass`: silently ignore errors and omit them from output.
- `warn`: omit them from output, but print a warning to stderr.
- `stop`: stop at the first error.
- `+1`: include the error in output by printing an error code in place of `{0|1}`:
  - Let `rc` be the per-file error code (prefer `std::io::Error::raw_os_error()` when present; otherwise use an internal negative `rc`).
  - Print `rc` verbatim if it is negative; otherwise print `rc+1` (to shift it out of `{0,1}`).

Process exit code policy (ripgrep-inspired):

- For "common filesystem access" per-file errors (open/read/metadata failures, directories encountered, etc.):
  - `+1` and `pass`: `0` if at least one file was successfully classified, else `2`.
  - `warn` and `stop`: `2` if any such error occurred, else `0`.
- For non-I/O fatal errors (invalid CLI, output write failures, internal invariants): `2`.

## 4) Work breakdown (suggested)

1. Create `Cargo.toml` + `src/main.rs` skeleton; implement CLI parser + options struct.
2. Implement input readers:
   - `--files-from`: read stdin/file; decode with BOM sniffing (ripgrep-style via `encoding_rs_io`); split on `\n` and trim trailing `\r`.
   - `--files-from0`: read stdin/file; decode with BOM sniffing (ripgrep-style via `encoding_rs_io`); split on `\0`.
3. Implement output path formatting (`--abs`) with good Windows behavior.
4. Implement glob filtering using `ignore::overrides::OverrideBuilder` (borrow ripgrep approach).
5. Implement classifier (read chunks; `memchr(0)`).
6. Emit output with configurable `fs`/`rs` as bytes.
7. Add unit tests + a couple of integration tests (golden outputs) for Windows + Unix-ish behavior where possible.

## 5) Open questions (please answer before we lock Phase 1)

None for Phase 1.

Notes:

- For now, `--files-from` / `--files-from0` decoding follows ripgrep-like BOM sniffing via `encoding_rs_io` (revisit later if needed).

## 6) ripgrep code borrowing

The intention is "copy + modify" for small, behavior-critical glue code, and "depend on the same crates" for the deeper libraries (so we inherit ripgrep's behavior without forking large subsystems).

Borrow via dependencies (preferred when it matches our needs):

- `ignore` (used by ripgrep for `-g/--glob` and `--iglob`):
  - ripgrep references: `..\ripgrep\crates\core\flags\hiargs.rs` (`fn globs`) and `..\ripgrep\crates\ignore\src\overrides.rs`
  - risbin use: implement `-g/-G/-i/-I` by building an `ignore::overrides::Override` matcher rooted at `cwd`
- `globset` (glob syntax engine used by `ignore`):
  - ripgrep reference: `..\ripgrep\crates\globset`
  - risbin use: indirectly via `ignore`
- `encoding_rs_io` (BOM sniffing/transcoding):
  - ripgrep reference: `..\ripgrep\crates\searcher\src\searcher\mod.rs` (DecodeReaderBytesBuilder configuration)
  - risbin use: decode `--files-from*` input streams similarly (BOM sniffing enabled, strip BOM)

Copy + modify (keep behavior close to ripgrep, but tailor to risbin's simpler CLI):

- Non-fatal error tracking + exit code shaping:
  - ripgrep reference: `..\ripgrep\crates\core\messages.rs` (errored flag + `err_message!`)
  - risbin adaptation: keep a global "saw error" flag, but map exit codes per `--error-flag` policy
- BOM-sniffing reader helper for `--files-from*`:
  - ripgrep reference: `..\ripgrep\crates\searcher\src\searcher\mod.rs` (DecodeReaderBytesBuilder config)
  - risbin adaptation: factor into a small `decode_reader(...)` helper used by `--files-from` and `--files-from0`

Deliberate non-borrows (Phase 1):

- Full ripgrep flag system (`crates/core/flags/*`) is too large for risbin's CLI; use `lexopt` and only borrow small parsing helpers if we later add complex flag semantics.
- Directory traversal/walker/haystack code (`crates/core/haystack.rs`, `ignore::WalkBuilder`) is out of scope for Phase 1 since risbin does not accept directories as inputs.

Tracking:

- Record the ripgrep git revision we borrowed from (e.g., as a comment in the vendored module header or a `VENDORED.md` file) so future syncing is mechanical.
