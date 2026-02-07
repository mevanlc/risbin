use std::{
    ffi::OsString,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Context;
use encoding_rs_io::DecodeReaderBytesBuilder;
use ignore::overrides::OverrideBuilder;
use path_absolutize::Absolutize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErrorFlagMode {
    Plus1,
    Pass,
    Warn,
    Stop,
}

impl ErrorFlagMode {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "+1" => Ok(Self::Plus1),
            "pass" => Ok(Self::Pass),
            "warn" => Ok(Self::Warn),
            "stop" => Ok(Self::Stop),
            _ => Err(format!(
                "invalid value for --error-flag: {s} (expected one of +1,pass,warn,stop)"
            )),
        }
    }
}

#[derive(Debug)]
enum InputSource {
    FilesFrom { path: OsString, nul: bool },
    Positional(Vec<OsString>),
}

#[derive(Debug)]
struct Opts {
    input: InputSource,
    abs: bool,
    fs: Vec<u8>,
    rs: Vec<u8>,
    error_flag: ErrorFlagMode,
    globs: Vec<String>,
    globs_implicit_negated: Vec<String>,
    iglobs: Vec<String>,
    iglobs_implicit_negated: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("risbin: {err}");
            ExitCode::from(2)
        }
    }
}

fn usage() -> &'static str {
    "Usage:\n\
  risbin [opts] --files-from <-|filelistfile>\n\
  risbin [opts] --files-from0 <-|file0listfile>\n\
  risbin [opts] <file> [file...]\n\
\n\
Output:\n\
  {0|1|rc_shifted}{fs}{path}{rs}...\n\
\n\
Options:\n\
  --abs                 Output absolute paths (formatting only)\n\
  --fs <str>            Field separator (default: TAB)\n\
  --rs <str>            Record separator (default: LF)\n\
  --fs0                 Field separator is NUL\n\
  --rs0                 Record separator is NUL\n\
  --error-flag <mode>   One of: +1, pass, warn, stop (default: warn)\n\
  -g, --glob <glob>     Add a glob (ripgrep-style)\n\
  -G <glob>             Like -g, but implicitly negated\n\
  -i, --iglob <glob>    Add a case-insensitive glob\n\
  -I <glob>             Like -i, but implicitly negated\n\
  -h, --help            Print help\n\
  -V, --version         Print version\n"
}

fn run() -> anyhow::Result<ExitCode> {
    let opts = parse_args()?;
    let cwd = std::env::current_dir()?;

    let overrides = build_overrides(&cwd, &opts)?;
    let mut classifier = risbin::Classifier::new();

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    let mut saw_success = false;
    let mut saw_error = false;
    let mut wrote_any_record = false;
    let mut saw_broken_pipe = false;

    let mut process_path = |path: PathBuf| -> anyhow::Result<bool> {
        if let Some(ref overrides) = overrides {
            let is_dir = false;
            if overrides.matched(&path, is_dir).is_ignore() {
                return Ok(true);
            }
        }

        match classify_one_path(&mut classifier, &path) {
            Ok(kind) => {
                saw_success = true;
                wrote_any_record = true;
                if let Err(err) = write_record(
                    &mut out,
                    kind.code_byte(),
                    &opts.fs,
                    format_path_for_output(&cwd, &path, opts.abs).as_bytes(),
                    &opts.rs,
                ) {
                    if err.kind() == io::ErrorKind::BrokenPipe {
                        saw_broken_pipe = true;
                        return Ok(false);
                    }
                    return Err(err.into());
                }
                Ok(true)
            }
            Err(err) => {
                saw_error = true;
                match opts.error_flag {
                    ErrorFlagMode::Pass => Ok(true),
                    ErrorFlagMode::Warn => {
                        eprintln!(
                            "risbin: {}: {}",
                            format_path_for_output(&cwd, &path, opts.abs),
                            err
                        );
                        Ok(true)
                    }
                    ErrorFlagMode::Stop => {
                        eprintln!(
                            "risbin: {}: {}",
                            format_path_for_output(&cwd, &path, opts.abs),
                            err
                        );
                        Ok(false)
                    }
                    ErrorFlagMode::Plus1 => {
                        wrote_any_record = true;
                        let code = error_code_shifted(&err).to_string();
                        if let Err(err) = write_record_bytes(
                            &mut out,
                            code.as_bytes(),
                            &opts.fs,
                            format_path_for_output(&cwd, &path, opts.abs).as_bytes(),
                            &opts.rs,
                        ) {
                            if err.kind() == io::ErrorKind::BrokenPipe {
                                saw_broken_pipe = true;
                                return Ok(false);
                            }
                            return Err(err.into());
                        }
                        Ok(true)
                    }
                }
            }
        }
    };

    match opts.input {
        InputSource::Positional(paths) => {
            for p in paths {
                let path = PathBuf::from(p);
                if !process_path(path)? {
                    break;
                }
            }
        }
        InputSource::FilesFrom { path, nul } => {
            if path == OsString::from("-") {
                let stdin = std::io::stdin();
                let locked = stdin.lock();
                let cont = process_paths_from_reader(
                    &cwd,
                    locked,
                    nul,
                    |path| process_path(path),
                )?;
                if !cont {
                    // stopped
                }
            } else {
                let file = std::fs::File::open(&path)
                    .with_context(|| format!("failed to open list file {}", path.to_string_lossy()))?;
                let cont =
                    process_paths_from_reader(&cwd, file, nul, |p| process_path(p))?;
                if !cont {
                    // stopped
                }
            }
        }
    }

    if saw_broken_pipe {
        return Ok(ExitCode::from(0));
    }
    if let Err(err) = out.flush() {
        if err.kind() == io::ErrorKind::BrokenPipe {
            return Ok(ExitCode::from(0));
        }
        return Err(err.into());
    }

    Ok(exit_code_from(saw_success, saw_error, wrote_any_record, opts.error_flag))
}

fn exit_code_from(
    saw_success: bool,
    saw_error: bool,
    wrote_any_record: bool,
    mode: ErrorFlagMode,
) -> ExitCode {
    if saw_error {
        match mode {
            ErrorFlagMode::Warn | ErrorFlagMode::Stop => ExitCode::from(2),
            ErrorFlagMode::Pass | ErrorFlagMode::Plus1 => {
                if saw_success {
                    ExitCode::from(0)
                } else {
                    ExitCode::from(2)
                }
            }
        }
    } else if wrote_any_record && saw_success {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    }
}

fn parse_args() -> anyhow::Result<Opts> {
    use lexopt::prelude::*;

    let mut abs = false;
    let mut fs = b"\t".to_vec();
    let mut rs = b"\n".to_vec();
    let mut error_flag = ErrorFlagMode::Warn;

    let mut files_from: Option<(OsString, bool)> = None;
    let mut positional: Vec<OsString> = vec![];

    let mut globs = vec![];
    let mut globs_implicit_negated = vec![];
    let mut iglobs = vec![];
    let mut iglobs_implicit_negated = vec![];

    let mut p = lexopt::Parser::from_env();
    while let Some(arg) = p.next()? {
        match arg {
            Value(v) => positional.push(v),
            Short('h') | Long("help") => {
                print!("{}", usage());
                std::process::exit(0);
            }
            Short('V') | Long("version") => {
                println!("risbin {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }

            Long("files-from") => {
                let v = p.value()?;
                if files_from.is_some() {
                    return Err(anyhow::anyhow!(
                        "cannot use both --files-from and --files-from0"
                    ));
                }
                files_from = Some((v, false));
            }
            Long("files-from0") => {
                let v = p.value()?;
                if files_from.is_some() {
                    return Err(anyhow::anyhow!(
                        "cannot use both --files-from and --files-from0"
                    ));
                }
                files_from = Some((v, true));
            }

            Long("abs") => abs = true,

            Long("fs") => {
                let v = p.value()?;
                fs = v.to_string_lossy().as_bytes().to_vec();
            }
            Long("rs") => {
                let v = p.value()?;
                rs = v.to_string_lossy().as_bytes().to_vec();
            }
            Long("fs0") => fs = vec![0],
            Long("rs0") => rs = vec![0],

            Long("error-flag") => {
                let v = p.value()?;
                error_flag = ErrorFlagMode::parse(&v.to_string_lossy())
                    .map_err(anyhow::Error::msg)?;
            }

            Short('g') | Long("glob") => {
                let v = p.value()?;
                globs.push(v.to_string_lossy().to_string());
            }
            Short('G') => {
                let v = p.value()?;
                globs_implicit_negated.push(v.to_string_lossy().to_string());
            }
            Short('i') | Long("iglob") => {
                let v = p.value()?;
                iglobs.push(v.to_string_lossy().to_string());
            }
            Short('I') => {
                let v = p.value()?;
                iglobs_implicit_negated.push(v.to_string_lossy().to_string());
            }

            _ => return Err(anyhow::anyhow!("unrecognized argument: {arg:?}")),
        }
    }

    let input = match (files_from, positional.is_empty()) {
        (Some((path, nul)), true) => InputSource::FilesFrom { path, nul },
        (Some(_), false) => {
            return Err(anyhow::anyhow!(
                "cannot mix --files-from/--files-from0 with positional paths"
            ))
        }
        (None, false) => InputSource::Positional(positional),
        (None, true) => return Err(anyhow::anyhow!("no input paths provided")),
    };

    Ok(Opts {
        input,
        abs,
        fs,
        rs,
        error_flag,
        globs,
        globs_implicit_negated,
        iglobs,
        iglobs_implicit_negated,
    })
}

fn build_overrides(cwd: &Path, opts: &Opts) -> anyhow::Result<Option<ignore::overrides::Override>> {
    let has_any = !opts.globs.is_empty()
        || !opts.globs_implicit_negated.is_empty()
        || !opts.iglobs.is_empty()
        || !opts.iglobs_implicit_negated.is_empty();
    if !has_any {
        return Ok(None);
    }

    let mut builder = OverrideBuilder::new(cwd);

    for g in &opts.globs {
        builder.add(g)?;
    }
    for g in &opts.globs_implicit_negated {
        if starts_with_unescaped_bang(g) {
            anyhow::bail!(
                "-G patterns must not start with unescaped '!': {g} (use -g instead)"
            );
        }
        builder.add(&format!("!{g}"))?;
    }

    builder.case_insensitive(true)?;
    for g in &opts.iglobs {
        builder.add(g)?;
    }
    for g in &opts.iglobs_implicit_negated {
        if starts_with_unescaped_bang(g) {
            anyhow::bail!(
                "-I patterns must not start with unescaped '!': {g} (use -i instead)"
            );
        }
        builder.add(&format!("!{g}"))?;
    }

    Ok(Some(builder.build()?))
}

fn starts_with_unescaped_bang(s: &str) -> bool {
    s.as_bytes().first() == Some(&b'!')
}

fn decode_reader<R: io::Read>(reader: R) -> io::Result<impl io::Read> {
    // Mirror ripgrep's approach (DecodeReaderBytesBuilder with BOM sniffing).
    let mut builder = DecodeReaderBytesBuilder::new();
    builder
        .utf8_passthru(true)
        .strip_bom(true)
        .bom_override(true)
        .bom_sniffing(true);
    Ok(builder.build(reader))
}

fn process_paths_from_reader<R: io::Read, F>(
    _cwd: &Path,
    reader: R,
    nul: bool,
    mut f: F,
) -> anyhow::Result<bool>
where
    F: FnMut(PathBuf) -> anyhow::Result<bool>,
{
    let decoder = decode_reader(reader)?;
    let mut br = io::BufReader::new(decoder);
    let delim = if nul { 0 } else { b'\n' };

    let mut buf = Vec::new();
    loop {
        buf.clear();
        let n = br.read_until(delim, &mut buf)?;
        if n == 0 {
            return Ok(true);
        }
        if !nul {
            if buf.last() == Some(&b'\n') {
                buf.pop();
            }
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        } else if buf.last() == Some(&0) {
            buf.pop();
        }
        if buf.is_empty() {
            continue;
        }
        let s = String::from_utf8_lossy(&buf);
        let cont = f(PathBuf::from(s.as_ref()))?;
        if !cont {
            return Ok(false);
        }
    }
}

fn classify_one_path(
    classifier: &mut risbin::Classifier,
    path: &Path,
) -> Result<risbin::Kind, io::Error> {
    // Enforce "no directories" early.
    if let Ok(md) = std::fs::metadata(path) {
        if md.is_dir() {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, "is a directory"));
        }
    }
    classifier.classify_path(path)
}

fn format_path_for_output(cwd: &Path, path: &Path, abs: bool) -> String {
    if !abs {
        return path.to_string_lossy().to_string();
    }
    match path.absolutize_from(cwd) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

fn write_record(
    out: &mut impl Write,
    code_byte: u8,
    fs: &[u8],
    path_bytes: &[u8],
    rs: &[u8],
) -> io::Result<()> {
    out.write_all(&[code_byte])?;
    out.write_all(fs)?;
    out.write_all(path_bytes)?;
    out.write_all(rs)?;
    Ok(())
}

fn write_record_bytes(
    out: &mut impl Write,
    code_bytes: &[u8],
    fs: &[u8],
    path_bytes: &[u8],
    rs: &[u8],
) -> io::Result<()> {
    out.write_all(code_bytes)?;
    out.write_all(fs)?;
    out.write_all(path_bytes)?;
    out.write_all(rs)?;
    Ok(())
}

#[derive(Debug)]
enum PerFileCode {
    Os(i32),
    Internal(i32),
}

fn per_file_code(err: &io::Error) -> PerFileCode {
    if let Some(rc) = err.raw_os_error() {
        return PerFileCode::Os(rc);
    }
    // Internal negative codes (kept small and stable-ish).
    let rc = match err.kind() {
        io::ErrorKind::IsADirectory => -1,
        io::ErrorKind::NotFound => -2,
        io::ErrorKind::PermissionDenied => -3,
        _ => -10,
    };
    PerFileCode::Internal(rc)
}

fn error_code_shifted(err: &io::Error) -> i64 {
    match per_file_code(err) {
        PerFileCode::Os(rc) => {
            if rc < 0 {
                rc as i64
            } else {
                (rc as i64) + 1
            }
        }
        PerFileCode::Internal(rc) => rc as i64,
    }
}
