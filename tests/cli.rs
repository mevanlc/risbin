use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::*;

fn write_file(path: &std::path::Path, bytes: &[u8]) {
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
}

fn risbin_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("risbin"))
}

#[test]
fn classify_two_files_default_separators() {
    let td = tempfile::tempdir().unwrap();
    let text = td.path().join("a.txt");
    let bin = td.path().join("b.bin");

    write_file(&text, b"hello\nworld\n");
    write_file(&bin, b"\x00abc");

    let mut cmd = risbin_cmd();
    cmd.arg(text.as_os_str()).arg(bin.as_os_str());

    let output = cmd.assert().success().get_output().stdout.clone();
    let out = String::from_utf8_lossy(&output);

    let expected = format!(
        "0\t{}\n1\t{}\n",
        text.to_string_lossy(),
        bin.to_string_lossy()
    );
    assert_eq!(expected, out);
}

#[test]
fn fs0_rs0_emit_nul_separators() {
    let td = tempfile::tempdir().unwrap();
    let text = td.path().join("a.txt");
    write_file(&text, b"hello");

    let mut cmd = risbin_cmd();
    cmd.arg("--fs0").arg("--rs0").arg(text.as_os_str());

    let out = cmd.assert().success().get_output().stdout.clone();
    let expected = {
        let mut v = Vec::new();
        v.extend_from_slice(b"0");
        v.push(0);
        v.extend_from_slice(text.to_string_lossy().as_bytes());
        v.push(0);
        v
    };
    assert_eq!(expected, out);
}

#[test]
fn files_from_newline() {
    let td = tempfile::tempdir().unwrap();
    let text = td.path().join("a.txt");
    let bin = td.path().join("b.bin");
    write_file(&text, b"hello");
    write_file(&bin, b"\x00abc");

    let list = td.path().join("list.txt");
    write_file(
        &list,
        format!("{}\n{}\n", text.to_string_lossy(), bin.to_string_lossy()).as_bytes(),
    );

    let mut cmd = risbin_cmd();
    cmd.arg("--files-from").arg(list.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!(
        "0\t{}\n1\t{}\n",
        text.to_string_lossy(),
        bin.to_string_lossy()
    );
    assert_eq!(expected, out);
}

#[test]
fn files_from_crlf() {
    let td = tempfile::tempdir().unwrap();
    let text = td.path().join("a.txt");
    let bin = td.path().join("b.bin");
    write_file(&text, b"hello");
    write_file(&bin, b"\x00abc");

    let list = td.path().join("list-crlf.txt");
    write_file(
        &list,
        format!(
            "{}\r\n{}\r\n",
            text.to_string_lossy(),
            bin.to_string_lossy()
        )
        .as_bytes(),
    );

    let mut cmd = risbin_cmd();
    cmd.arg("--files-from").arg(list.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!(
        "0\t{}\n1\t{}\n",
        text.to_string_lossy(),
        bin.to_string_lossy()
    );
    assert_eq!(expected, out);
}

#[test]
fn files_from0_nul_delimited() {
    let td = tempfile::tempdir().unwrap();
    let text = td.path().join("a.txt");
    let bin = td.path().join("b.bin");
    write_file(&text, b"hello");
    write_file(&bin, b"\x00abc");

    let list = td.path().join("list0.bin");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(text.to_string_lossy().as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(bin.to_string_lossy().as_bytes());
    bytes.push(0);
    write_file(&list, &bytes);

    let mut cmd = risbin_cmd();
    cmd.arg("--files-from0").arg(list.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!(
        "0\t{}\n1\t{}\n",
        text.to_string_lossy(),
        bin.to_string_lossy()
    );
    assert_eq!(expected, out);
}

#[test]
fn glob_filters_paths() {
    let td = tempfile::tempdir().unwrap();
    let md = td.path().join("a.md");
    let txt = td.path().join("b.txt");
    write_file(&md, b"hello");
    write_file(&txt, b"hello");

    let mut cmd = risbin_cmd();
    cmd.arg("-g").arg("*.md").arg(md.as_os_str()).arg(txt.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!("0\t{}\n", md.to_string_lossy());
    assert_eq!(expected, out);
}

#[test]
fn glob_implicit_negation_excludes() {
    let td = tempfile::tempdir().unwrap();
    let md = td.path().join("a.md");
    let txt = td.path().join("b.txt");
    write_file(&md, b"hello");
    write_file(&txt, b"hello");

    let mut cmd = risbin_cmd();
    cmd.arg("-G").arg("*.md").arg(md.as_os_str()).arg(txt.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!("0\t{}\n", txt.to_string_lossy());
    assert_eq!(expected, out);
}

#[test]
fn iglob_is_case_insensitive() {
    let td = tempfile::tempdir().unwrap();
    let md = td.path().join("a.md");
    write_file(&md, b"hello");

    let mut cmd = risbin_cmd();
    cmd.arg("-i").arg("*.MD").arg(md.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!("0\t{}\n", md.to_string_lossy());
    assert_eq!(expected, out);
}

#[test]
fn error_flag_warn_omits_output_and_exits_2() {
    let td = tempfile::tempdir().unwrap();
    let ok = td.path().join("a.txt");
    write_file(&ok, b"hello");
    let missing = td.path().join("missing.txt");

    let mut cmd = risbin_cmd();
    cmd.arg(missing.as_os_str()).arg(ok.as_os_str());

    cmd.assert()
        .code(2)
        .stdout(predicate::str::contains(ok.to_string_lossy().as_ref()))
        .stdout(predicate::str::contains(missing.to_string_lossy().as_ref()).not())
        .stderr(predicate::str::contains("risbin:"));
}

#[test]
fn error_flag_pass_omits_output_and_exits_0_if_any_success() {
    let td = tempfile::tempdir().unwrap();
    let ok = td.path().join("a.txt");
    write_file(&ok, b"hello");
    let missing = td.path().join("missing.txt");

    let mut cmd = risbin_cmd();
    cmd.arg("--error-flag")
        .arg("pass")
        .arg(missing.as_os_str())
        .arg(ok.as_os_str());

    cmd.assert()
        .code(0)
        .stdout(predicate::str::contains(ok.to_string_lossy().as_ref()))
        .stdout(predicate::str::contains(missing.to_string_lossy().as_ref()).not())
        .stderr(predicate::str::is_empty());
}

#[test]
fn error_flag_stop_stops_at_first_error() {
    let td = tempfile::tempdir().unwrap();
    let ok = td.path().join("a.txt");
    write_file(&ok, b"hello");
    let missing = td.path().join("missing.txt");

    let mut cmd = risbin_cmd();
    cmd.arg("--error-flag")
        .arg("stop")
        .arg(missing.as_os_str())
        .arg(ok.as_os_str());

    cmd.assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("risbin:"));
}
