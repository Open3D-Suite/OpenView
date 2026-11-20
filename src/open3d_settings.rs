// SPDX-License-Identifier: GPL-3.0-or-later
//! The language shared by every OPEN3D app: one preference, in `settings.json` (D-050 to D-058).
//!
//! This file is a copy. Its original is `common/settings/open3d_settings.rs` in the OPEN3D root,
//! which also holds the contract it keeps (`common/settings.md`): change it there, never here.
//!
//! The file is `%APPDATA%\Open3D\settings.json`, or `settings.json` in the folder that
//! `OPEN3D_SETTINGS_DIR` names. It is a JSON object whose `"language"` is `"en"` or `"es"`.
//! Reading never fails and never writes: no file, no key, a file that cannot be read and any other
//! value all mean "no preference yet", and then the language of Windows decides (Spanish if it is
//! Spanish, otherwise English). Writing keeps every other key, and happens only when a person
//! changes the language.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The environment variable that, when set, names the folder in place of `%APPDATA%\Open3D`.
pub const DIR_VARIABLE: &str = "OPEN3D_SETTINGS_DIR";
/// The file's name in that folder.
pub const FILE_NAME: &str = "settings.json";
/// The key of the language in the file.
const KEY: &str = "language";

/// A language every OPEN3D app can be shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Spanish,
}

impl Language {
    /// The code the file holds: `en` or `es`.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
        }
    }

    /// The language of a code; `None` for anything but `en` and `es`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en" => Some(Self::English),
            "es" => Some(Self::Spanish),
            _ => None,
        }
    }
}

/// The folder of the shared settings: the one `OPEN3D_SETTINGS_DIR` names, otherwise
/// `%APPDATA%\Open3D`. `None` when neither variable is set.
pub fn settings_dir() -> Option<PathBuf> {
    settings_dir_from(std::env::var_os(DIR_VARIABLE), std::env::var_os("APPDATA"))
}

/// [`settings_dir`] from the values of `OPEN3D_SETTINGS_DIR` and `APPDATA`. An empty value counts
/// as not set.
pub fn settings_dir_from(variable: Option<OsString>, appdata: Option<OsString>) -> Option<PathBuf> {
    let set = |value: Option<OsString>| value.filter(|value| !value.is_empty()).map(PathBuf::from);
    set(variable).or_else(|| set(appdata).map(|appdata| appdata.join("Open3D")))
}

/// The language to show: the preference in `dir`, or the language of Windows when there is none.
pub fn language(dir: Option<&Path>) -> Language {
    dir.and_then(read_language).unwrap_or_else(windows_language)
}

/// The preference kept in `dir`; `None` when there is none yet. It never fails and never writes.
pub fn read_language(dir: &Path) -> Option<Language> {
    language_in(&std::fs::read(dir.join(FILE_NAME)).ok()?)
}

/// The preference in the bytes of a `settings.json`.
pub fn language_in(content: &[u8]) -> Option<Language> {
    match parse(content)?.get(KEY)? {
        serde_json::Value::String(code) => Language::from_code(code),
        _ => None,
    }
}

/// Keeps `language` in `dir` as the shared preference; `None` takes the preference out, and then
/// the language of Windows decides. Every other key of the file stays. The folder is created if
/// it is missing, and the file is written whole into a new file beside it, which then takes its
/// place. Call it only when a person changes the language. The error says what failed.
pub fn write_language(dir: &Path, language: Option<Language>) -> Result<(), String> {
    let path = dir.join(FILE_NAME);
    let old = match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        // Its other keys could not be kept.
        Err(e) => return Err(format!("couldn't read {} ({e})", path.display())),
    };
    let new = with_language(old.as_deref(), language)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {} ({e})", dir.display()))?;
    replace(&path, &new)
}

/// What a `settings.json` holds after [`write_language`]: every key of `old` and the language, or
/// no `language` key for `None`. An `old` that is not a JSON object is not kept.
pub fn with_language(old: Option<&[u8]>, language: Option<Language>) -> Result<Vec<u8>, String> {
    let mut object = old.and_then(parse).unwrap_or_default();
    match language {
        Some(language) => {
            object.insert(KEY.to_string(), serde_json::Value::String(language.code().to_string()));
        }
        None => {
            object.remove(KEY);
        }
    }
    let mut bytes = serde_json::to_vec_pretty(&serde_json::Value::Object(object)).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The JSON object in the bytes of a `settings.json`: UTF-8, with or without a byte order mark.
fn parse(content: &[u8]) -> Option<serde_json::Map<String, serde_json::Value>> {
    let text = std::str::from_utf8(content).ok()?;
    match serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)).ok()? {
        serde_json::Value::Object(object) => Some(object),
        _ => None,
    }
}

/// Writes `bytes` into a new file in the folder of `path`, which then takes the place of `path`:
/// another app never reads half a file.
fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let dir = path.parent().unwrap_or(Path::new("."));
    let (temporary, mut file) = new_file(dir).map_err(|e| format!("couldn't create a file in {} ({e})", dir.display()))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    match written.and_then(|()| std::fs::rename(&temporary, path)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(format!("couldn't write {} ({e})", path.display()))
        }
    }
}

/// A new file of its own in `dir`, for [`replace`].
fn new_file(dir: &Path) -> std::io::Result<(PathBuf, std::fs::File)> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos());
    for attempt in 0..100 {
        let path = dir.join(format!("{FILE_NAME}.{}-{stamp}-{attempt}.tmp", std::process::id()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "every name tried is taken"))
}

/// The language of Windows (D-052): Spanish when the user's interface language is Spanish, of
/// any country; otherwise English.
pub fn windows_language() -> Language {
    language_for_windows(user_interface_language())
}

/// [`windows_language`] for a Windows language identifier (a LANGID): Spanish when its primary
/// language is Spanish.
pub fn language_for_windows(langid: u16) -> Language {
    const PRIMARY_LANGUAGE: u16 = 0x03ff;
    const LANG_SPANISH: u16 = 0x0a;
    if langid & PRIMARY_LANGUAGE == LANG_SPANISH { Language::Spanish } else { Language::English }
}

/// The user's interface language, as `GetUserDefaultUILanguage` gives it.
// `unsafe` is allowed here because this function calls Windows itself.
#[cfg(windows)]
#[allow(unsafe_code)]
fn user_interface_language() -> u16 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // SAFETY: a plain Win32 call with no arguments; it cannot fail.
    unsafe { GetUserDefaultUILanguage() }
}

/// Elsewhere there is no Windows to ask: English.
#[cfg(not(windows))]
fn user_interface_language() -> u16 {
    0
}

#[cfg(test)]
mod tests {
    //! The shared cases (`open3d_settings_cases.txt`, beside this file) and what only Rust has.

    use super::*;

    const CASES: &str = include_str!("open3d_settings_cases.txt");

    /// The cases of one kind: the line number and the fields after the kind.
    fn cases(kind: &str) -> Vec<(usize, Vec<&'static str>)> {
        let found: Vec<_> = CASES
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
            .map(|(number, line)| (number + 1, line.split('|').map(|field| field.trim_matches(' ')).collect::<Vec<_>>()))
            .filter(|(_, fields)| fields[0] == kind)
            .map(|(number, fields)| (number, fields[1..].to_vec()))
            .collect();
        assert!(!found.is_empty(), "no {kind} cases");
        found
    }

    /// The bytes a field stands for; `None` for "(no file)".
    fn content(field: &str) -> Option<Vec<u8>> {
        match field {
            "(no file)" => return None,
            "(empty)" => return Some(Vec::new()),
            _ => {}
        }
        let mut bytes = Vec::new();
        let mut rest = field.as_bytes();
        while let Some((&byte, after)) = rest.split_first() {
            rest = after;
            if byte != b'\\' {
                bytes.push(byte);
                continue;
            }
            let (&escape, after) = rest.split_first().expect("an escape at the end of a field");
            rest = after;
            match escape {
                b'n' => bytes.push(b'\n'),
                b'r' => bytes.push(b'\r'),
                b't' => bytes.push(b'\t'),
                b'\\' => bytes.push(b'\\'),
                b'x' => {
                    let hex = std::str::from_utf8(&rest[..2]).expect("two hexadecimal digits");
                    bytes.push(u8::from_str_radix(hex, 16).expect("two hexadecimal digits"));
                    rest = &rest[2..];
                }
                other => panic!("unknown escape \\{}", other as char),
            }
        }
        Some(bytes)
    }

    fn langid(field: &str) -> u16 {
        u16::from_str_radix(field.trim_start_matches("0x"), 16).expect("a LANGID in hexadecimal")
    }

    fn preference(field: &str) -> Option<Language> {
        if field == "none" { None } else { Some(Language::from_code(field).expect("en, es or none")) }
    }

    /// A folder of its own, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("open3d-settings-rs-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        /// The folder with `settings.json` holding `content`, or without it.
        fn with(name: &str, content: Option<&[u8]>) -> Self {
            let scratch = Self::new(name);
            if let Some(content) = content {
                std::fs::write(scratch.0.join(FILE_NAME), content).unwrap();
            }
            scratch
        }

        fn names(&self) -> Vec<String> {
            std::fs::read_dir(&self.0).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_language_of_windows() {
        for (line, fields) in cases("windows") {
            assert_eq!(Some(language_for_windows(langid(fields[0]))), preference(fields[1]), "line {line}");
        }
    }

    #[test]
    fn reading() {
        for (line, fields) in cases("read") {
            let scratch = Scratch::with(&format!("read-{line}"), content(fields[0]).as_deref());
            let found = read_language(&scratch.0);
            assert_eq!(found, preference(fields[2]), "line {line}: the preference");
            let shown = found.unwrap_or_else(|| language_for_windows(langid(fields[1])));
            assert_eq!(Some(shown), preference(fields[3]), "line {line}: the language shown");
            assert_eq!(scratch.names(), if fields[0] == "(no file)" { vec![] } else { vec![FILE_NAME.to_string()] }, "line {line}");
        }
    }

    #[test]
    fn writing() {
        for (line, fields) in cases("write") {
            let scratch = Scratch::with(&format!("write-{line}"), content(fields[0]).as_deref());
            let language = preference(fields[1]);
            assert_eq!(write_language(&scratch.0, language), Ok(()), "line {line}");
            let written = std::fs::read(scratch.0.join(FILE_NAME)).unwrap();
            let text = std::str::from_utf8(&written).unwrap_or_else(|e| panic!("line {line}: {e}"));
            assert!(!text.starts_with('\u{feff}') && text.ends_with('\n'), "line {line}: {text:?}");
            let expected: serde_json::Value = serde_json::from_slice(&content(fields[2]).unwrap()).unwrap();
            let got: serde_json::Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("line {line}: {e}: {text:?}"));
            assert_eq!(got, expected, "line {line}");
            assert_eq!(read_language(&scratch.0), language, "line {line}");
            assert_eq!(scratch.names(), [FILE_NAME], "line {line}: nothing else is left in the folder");
        }
    }

    #[test]
    fn the_folder_is_the_variable_then_appdata() {
        let os = |text: &str| Some(OsString::from(text));
        assert_eq!(settings_dir_from(os("D:\\Suite\\Settings"), os("C:\\Roaming")), Some(PathBuf::from("D:\\Suite\\Settings")));
        assert_eq!(settings_dir_from(None, os("C:\\Roaming")), Some(Path::new("C:\\Roaming").join("Open3D")));
        assert_eq!(settings_dir_from(os(""), os("C:\\Roaming")), Some(Path::new("C:\\Roaming").join("Open3D")));
        assert_eq!(settings_dir_from(None, os("")), None);
        assert_eq!(settings_dir_from(None, None), None);
    }

    #[test]
    fn writing_creates_the_folder() {
        let scratch = Scratch::new("create");
        let dir = scratch.0.join("Open3D");
        assert_eq!(write_language(&dir, Some(Language::Spanish)), Ok(()));
        assert_eq!(read_language(&dir), Some(Language::Spanish));
        assert_eq!(language(Some(&dir)), Language::Spanish);
    }

    #[test]
    fn a_write_that_fails_says_so_and_changes_nothing() {
        // The folder is a file: it cannot be created.
        let scratch = Scratch::new("fail-folder");
        let blocked = scratch.0.join("Open3D");
        std::fs::write(&blocked, b"a file").unwrap();
        assert!(write_language(&blocked, Some(Language::Spanish)).is_err());
        assert_eq!(std::fs::read(&blocked).unwrap(), b"a file");
        // settings.json is a folder: it can be neither read nor replaced.
        let scratch = Scratch::new("fail-file");
        std::fs::create_dir(scratch.0.join(FILE_NAME)).unwrap();
        assert_eq!(read_language(&scratch.0), None);
        assert!(write_language(&scratch.0, Some(Language::English)).is_err());
        assert_eq!(scratch.names(), [FILE_NAME]);
    }

    #[test]
    fn without_a_folder_windows_decides() {
        assert_eq!(language(None), windows_language());
    }
}
