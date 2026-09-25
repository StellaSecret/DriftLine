//! The save: its text format, the desktop file, and that the web page provides what the game imports.
use std::collections::BTreeSet;
use std::path::PathBuf;
use vfield::save::{file, Save, DEFAULT_GRID, GRID_MODES};

fn ids(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// A fresh empty folder under the system temp dir.
fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vfield-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_new_player_has_nothing_done_and_the_default_settings() {
    let s = Save::default();
    assert!(s.done.is_empty() && !s.hc && s.grid == DEFAULT_GRID);
    assert_eq!(Save::parse(""), s);
}

#[test]
fn a_save_reads_back_exactly_what_was_written() {
    let s = Save { done: ids(&["pos-1", "pos-2", "sur-5", "cmb-3"]), grid: 3, hc: true };
    assert_eq!(Save::parse(&s.render()), s);
    assert!(s.render().starts_with("vfield-save 1\n"));
    // and the text is the same for the same state, whatever order levels were finished in
    let t = Save { done: ids(&["sur-5", "cmb-3", "pos-2", "pos-1"]), grid: 3, hc: true };
    assert_eq!(s.render(), t.render());
}

#[test]
fn the_old_progress_file_is_read_as_finished_levels() {
    let s = Save::parse("pos-1\npos-2\r\n\n  sur-1  \n");
    assert_eq!(s.done, ids(&["pos-1", "pos-2", "sur-1"]));
    assert_eq!((s.grid, s.hc), (DEFAULT_GRID, false));
}

#[test]
fn a_damaged_or_newer_save_is_read_as_far_as_it_goes() {
    let text = "\u{feff}vfield-save 1\ndone pos-1\ndone bad id\ndone \nfuture 7\ngrid banana\nhc 1\ndone sur-9\ngrid 99\nnonsense\n";
    let s = Save::parse(text);
    // the ids this build has never heard of are kept (a later build may know them); malformed lines are skipped
    assert_eq!(s.done, ids(&["pos-1", "sur-9"]));
    assert_eq!(s.grid, GRID_MODES - 1, "an out-of-range density is clamped");
    assert!(s.hc);
    // a level id cannot carry anything but its own characters
    assert!(Save::parse("vfield-save 1\ndone ../../etc\ndone a;b\n").done.is_empty());
    assert!(Save::parse("<script>\n").done.is_empty());
}

#[test]
fn every_real_level_id_survives_the_format() {
    let all: BTreeSet<String> = vfield::levels::all().iter().map(|l| l.id.to_string()).collect();
    let s = Save { done: all.clone(), ..Save::default() };
    assert_eq!(Save::parse(&s.render()).done, all);
}

#[test]
fn the_save_folder_follows_the_platform_and_an_override() {
    let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    let p = |s: &str| Some(PathBuf::from(s));
    assert_eq!(file::save_dir(&env(&[("VFIELD_SAVE_DIR", "/x"), ("HOME", "/h")]), false, false), p("/x"));
    assert_eq!(file::save_dir(&env(&[("HOME", "/h")]), false, false), p("/h/.local/share/vfield"));
    assert_eq!(file::save_dir(&env(&[("HOME", "/h"), ("XDG_DATA_HOME", "/d")]), false, false), p("/d/vfield"));
    assert_eq!(file::save_dir(&env(&[("HOME", "/h")]), false, true), p("/h/Library/Application Support/vfield"));
    assert_eq!(file::save_dir(&env(&[("APPDATA", "/a")]), true, false), p("/a/vfield"));
    assert_eq!(file::save_dir(&env(&[("HOME", "")]), false, false), None, "empty values count as unset");
    assert_eq!(file::save_dir(&env(&[]), false, false), None);
}

#[test]
fn the_file_is_written_whole_replaced_and_read_back() {
    let dir = temp("roundtrip").join("deeper"); // the folder does not exist yet
    let nowhere = temp("nolegacy");
    assert_eq!(file::read_from(&dir, &nowhere), None);
    let a = Save { done: ids(&["pos-1"]), ..Save::default() };
    assert!(file::write_to(&dir, &a.render()));
    let b = Save { done: ids(&["pos-1", "pos-2"]), grid: 0, hc: true };
    assert!(file::write_to(&dir, &b.render()));
    assert_eq!(Save::parse(&file::read_from(&dir, &nowhere).unwrap()), b);
    assert!(!dir.join(format!("{}.tmp", file::FILE)).exists(), "no half-written sibling is left behind");
}

#[test]
fn progress_from_an_earlier_build_is_picked_up_until_there_is_a_new_save() {
    let (dir, legacy) = (temp("new"), temp("old"));
    std::fs::write(legacy.join(file::LEGACY_FILE), "pos-1\npre-1\n").unwrap();
    let first = Save::parse(&file::read_from(&dir, &legacy).unwrap());
    assert_eq!(first.done, ids(&["pos-1", "pre-1"]));
    // once the new file exists it wins, and the old one is left alone
    assert!(file::write_to(&dir, &Save { done: ids(&["pos-1", "pre-1", "pos-2"]), ..first }.render()));
    assert_eq!(Save::parse(&file::read_from(&dir, &legacy).unwrap()).done.len(), 3);
    assert_eq!(std::fs::read_to_string(legacy.join(file::LEGACY_FILE)).unwrap(), "pos-1\npre-1\n");
}

#[test]
fn a_save_that_cannot_be_written_says_so() {
    let d = temp("blocked");
    let in_the_way = d.join("file");
    std::fs::write(&in_the_way, "x").unwrap();
    assert!(!file::write_to(&in_the_way.join("sub"), "text"), "a folder cannot be made under a file");
}

#[test]
fn the_web_page_provides_every_storage_function_the_game_imports() {
    let root = env!("CARGO_MANIFEST_DIR");
    let rust = std::fs::read_to_string(format!("{root}/src/storage.rs")).unwrap();
    let page = std::fs::read_to_string(format!("{root}/web/index.html")).unwrap();
    for f in ["vf_storage_len", "vf_storage_read", "vf_storage_write"] {
        assert!(rust.contains(&format!("fn {f}(")), "{f} is not declared by the game");
        assert!(page.contains(&format!("importObject.env.{f} =")), "{f} is not provided by web/index.html");
    }
    // the plugin must be registered before gl.js's load() runs
    assert!(page.find("miniquad_add_plugin").unwrap() < page.find("load(\"vfield.wasm\")").unwrap());
}
