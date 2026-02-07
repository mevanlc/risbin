use std::io::Write;

use assert_cmd::Command;

fn risbin_cmd() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("risbin"))
}

fn write_file(path: &std::path::Path, bytes: &[u8]) {
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
}

// Derived from ripgrep's tests/binary.rs, which uses include_bytes! on
// tests/data/sherlock-nul.txt to validate binary detection beyond the initial
// 64KiB read buffer.
const SHERLOCK_NUL: &[u8] = include_bytes!("data/sherlock-nul.txt");

#[test]
fn upstream_ripgrep_sherlock_nul_is_binary() {
    let td = tempfile::tempdir().unwrap();
    let hay = td.path().join("hay");
    write_file(&hay, SHERLOCK_NUL);

    let mut cmd = risbin_cmd();
    cmd.arg(hay.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!("1\t{}\n", hay.to_string_lossy());
    assert_eq!(expected, out);
}

// Derived from ripgrep's tests/feature.rs (f1_utf16_auto): UTF-16LE (with BOM)
// should be decoded via BOM sniffing and treated as text.
#[test]
fn upstream_ripgrep_utf16le_bom_is_text() {
    let td = tempfile::tempdir().unwrap();
    let p = td.path().join("utf16le.txt");

    // "Шерлок Холмс" encoded as UTF-16LE with BOM (from ripgrep tests).
    let bytes = b"\xff\xfe(\x045\x04@\x04;\x04>\x04:\x04 \x00%\x04>\x04;\x04<\x04A\x04";
    write_file(&p, bytes);

    let mut cmd = risbin_cmd();
    cmd.arg(p.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!("0\t{}\n", p.to_string_lossy());
    assert_eq!(expected, out);
}

#[test]
fn upstream_ripgrep_utf16be_bom_is_text() {
    let td = tempfile::tempdir().unwrap();
    let p = td.path().join("utf16be.txt");

    // UTF-16BE with BOM, containing "A".
    write_file(&p, b"\xFE\xFF\x00\x41");

    let mut cmd = risbin_cmd();
    cmd.arg(p.as_os_str());

    let assert = cmd.assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    let expected = format!("0\t{}\n", p.to_string_lossy());
    assert_eq!(expected, out);
}
