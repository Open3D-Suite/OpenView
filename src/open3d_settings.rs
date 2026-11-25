// SPDX-License-Identifier: GPL-3.0-or-later
//! What every OPEN3D app shares: the language, one preference in `settings.json`,
//! and the folders each app keeps its own files in.
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
//!
//! An app keeps its settings in `%APPDATA%\Open3D\<app>` ([`app_settings_dir`]) and everything else
//! it writes in `%LOCALAPPDATA%\Open3D\<app>` ([`app_data_dir`]), or in `<app>` inside the folders
//! that `OPEN3D_SETTINGS_DIR` and `OPEN3D_DATA_DIR` name. At start, before it reads its settings, it
//! copies the folders it used before into the new ones with [`carry_over`]: once, whole, and
//! leaving the old ones as they were.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The environment variable that, when set, names the folder in place of `%APPDATA%\Open3D`.
pub const DIR_VARIABLE: &str = "OPEN3D_SETTINGS_DIR";
/// The environment variable that, when set, names the folder in place of `%LOCALAPPDATA%\Open3D`.
pub const DATA_DIR_VARIABLE: &str = "OPEN3D_DATA_DIR";
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
    open3d_dir(variable, appdata)
}

/// The folder of the other files of every app (caches, logs, recovery files, databases): the one
/// `OPEN3D_DATA_DIR` names, otherwise `%LOCALAPPDATA%\Open3D`. `None` when neither variable is set.
pub fn data_dir() -> Option<PathBuf> {
    data_dir_from(std::env::var_os(DATA_DIR_VARIABLE), std::env::var_os("LOCALAPPDATA"))
}

/// [`data_dir`] from the values of `OPEN3D_DATA_DIR` and `LOCALAPPDATA`. An empty value counts as
/// not set.
pub fn data_dir_from(variable: Option<OsString>, localappdata: Option<OsString>) -> Option<PathBuf> {
    open3d_dir(variable, localappdata)
}

/// The folder a variable names, otherwise `Open3D` in the folder `base` names.
fn open3d_dir(variable: Option<OsString>, base: Option<OsString>) -> Option<PathBuf> {
    let set = |value: Option<OsString>| value.filter(|value| !value.is_empty()).map(PathBuf::from);
    set(variable).or_else(|| set(base).map(|base| base.join("Open3D")))
}

/// The folder of an app's own settings: `app` in [`settings_dir`], such as
/// `%APPDATA%\Open3D\OpenRef`. `None` when there is no [`settings_dir`].
pub fn app_settings_dir(app: &str) -> Option<PathBuf> {
    settings_dir().map(|dir| dir.join(app))
}

/// The folder of everything else an app writes: `app` in [`data_dir`], such as
/// `%LOCALAPPDATA%\Open3D\OpenRef`. `None` when there is no [`data_dir`].
pub fn app_data_dir(app: &str) -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(app))
}

/// What [`carry_over`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CarryOver {
    /// Nothing to do: the new folder is there already, or there is no old folder.
    Nothing,
    /// The old folder was copied into the new one: how many files, and what was left out (a file
    /// that could not be read, such as one in use, or a link), each with its path inside the old
    /// folder and why.
    Copied { files: usize, left_out: Vec<(PathBuf, String)> },
    /// Nothing was put in place and the new folder is still missing, so the next start tries
    /// again: why.
    Failed(String),
}

/// Copies an app's old folder into its new one, the first time it starts with the new folders.
/// Only when nothing has the name `new` and `old` is a folder: everything in `old`
/// is copied into a new folder beside `new`, which takes the name `new` once the copy is complete,
/// so a copy cut half way is never taken for a finished one. An existing `new` is never touched, and
/// `old` is never changed, moved or deleted. Links inside `old` are not followed. A file that cannot
/// be read is left out and said; the rest is still copied. Call it at start, before the app reads
/// its settings. It never panics.
pub fn carry_over(old: &Path, new: &Path) -> CarryOver {
    if exists(new) || !old.is_dir() {
        return CarryOver::Nothing;
    }
    let absolute = |path: &Path| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if absolute(new).starts_with(absolute(old)) {
        return CarryOver::Failed(format!("{} is inside {}", new.display(), old.display()));
    }
    let (Some(parent), Some(name)) = (new.parent(), new.file_name()) else {
        return CarryOver::Failed(format!("{} cannot be a folder of its own", new.display()));
    };
    let parent = if parent.as_os_str().is_empty() { Path::new(".") } else { parent };
    let temporary = match std::fs::create_dir_all(parent).and_then(|()| new_folder(parent, name)) {
        Ok(temporary) => temporary,
        Err(e) => return CarryOver::Failed(format!("couldn't create a folder in {} ({e})", parent.display())),
    };
    let mut copy = Copying { files: 0, left_out: Vec::new() };
    match copy.tree(old, &temporary, Path::new("")).and_then(|()| put_in_place(&temporary, new)) {
        Ok(()) => CarryOver::Copied { files: copy.files, left_out: copy.left_out },
        Err(reason) => {
            let _ = std::fs::remove_dir_all(&temporary);
            // Another start of the same app may have put its own copy in place meanwhile.
            if exists(new) { CarryOver::Nothing } else { CarryOver::Failed(reason) }
        }
    }
}

/// Whether something has the name `path`: a folder, a file or a link. When that cannot be told, it
/// counts as there, so that it is never touched.
fn exists(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(e) => !matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory),
    }
}

/// A new, empty folder of its own in `dir`, named after `name`, for [`carry_over`] to copy into.
fn new_folder(dir: &Path, name: &OsStr) -> std::io::Result<PathBuf> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos());
    for attempt in 0..100 {
        let mut folder = name.to_os_string();
        folder.push(format!(".carry-{}-{stamp}-{attempt}.tmp", std::process::id()));
        let path = dir.join(folder);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "every name tried is taken"))
}

/// Gives the finished copy its name, `new`. While nothing else has that name it tries a few times:
/// another program, such as an antivirus, may hold the copy for a moment.
fn put_in_place(temporary: &Path, new: &Path) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..10 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if exists(new) {
            return Err(format!("{} appeared while the copy was made", new.display()));
        }
        match std::fs::rename(temporary, new) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("couldn't rename {} to {} ({last})", temporary.display(), new.display()))
}

/// The files [`carry_over`] has copied so far, and what it left out.
struct Copying {
    files: usize,
    left_out: Vec<(PathBuf, String)>,
}

impl Copying {
    /// Copies what the folder `from` holds into the empty folder `to`; `inside` is the path of
    /// `from` inside the old folder. The error is one that stops the copy: the old folder itself
    /// cannot be read, or something cannot be written.
    fn tree(&mut self, from: &Path, to: &Path, inside: &Path) -> Result<(), String> {
        let entries = match std::fs::read_dir(from) {
            Ok(entries) => entries,
            Err(e) if inside.as_os_str().is_empty() => return Err(format!("couldn't read {} ({e})", from.display())),
            Err(e) => {
                self.left_out.push((inside.to_path_buf(), e.to_string()));
                return Ok(());
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    self.left_out.push((inside.to_path_buf(), e.to_string()));
                    continue;
                }
            };
            let name = entry.file_name();
            let (path, target) = (inside.join(&name), to.join(&name));
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    std::fs::create_dir(&target).map_err(|e| couldnt_write(&target, &e))?;
                    self.tree(&entry.path(), &target, &path)?;
                }
                Ok(kind) if kind.is_file() => match copy_file(&entry.path(), &target)? {
                    None => self.files += 1,
                    Some(why) => self.left_out.push((path, why)),
                },
                Ok(_) => self.left_out.push((path, "not a plain file or folder, so not copied".to_string())),
                Err(e) => self.left_out.push((path, e.to_string())),
            }
        }
        Ok(())
    }
}

/// Copies the file `from` into the new file `to`, with its contents and the time it was last
/// changed. `Ok(Some(why))` when `from` cannot be read; then `to` is not left behind. The error is
/// one of writing, which stops the copy. `from` is opened as other programs may still use it.
fn copy_file(from: &Path, to: &Path) -> Result<Option<String>, String> {
    use std::io::{Read, Write};
    let mut source = match std::fs::File::open(from) {
        Ok(source) => source,
        Err(e) => return Ok(Some(e.to_string())),
    };
    let modified = source.metadata().and_then(|metadata| metadata.modified()).ok();
    let mut target = std::fs::OpenOptions::new().write(true).create_new(true).open(to).map_err(|e| couldnt_write(to, &e))?;
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                drop(target);
                let _ = std::fs::remove_file(to);
                return Ok(Some(e.to_string()));
            }
        };
        target.write_all(buffer.get(..read).unwrap_or_default()).map_err(|e| couldnt_write(to, &e))?;
    }
    if let Some(modified) = modified {
        let _ = target.set_modified(modified);
    }
    target.sync_all().map_err(|e| couldnt_write(to, &e))?;
    Ok(None)
}

fn couldnt_write(path: &Path, error: &std::io::Error) -> String {
    format!("couldn't write {} ({error})", path.display())
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

/// The language of Windows: Spanish when the user's interface language is Spanish, of
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

    /// The value of a variable in a `folders` case.
    fn variable(field: &str) -> Option<OsString> {
        match field {
            "(unset)" => None,
            "(empty)" => Some(OsString::new()),
            value => Some(OsString::from(value)),
        }
    }

    #[test]
    fn the_folders_of_an_app() {
        for (line, fields) in cases("folders") {
            let folder = |field: &str| (field != "(none)").then(|| field.split('/').collect::<PathBuf>());
            let app = fields[0];
            let settings = settings_dir_from(variable(fields[1]), variable(fields[2])).map(|dir| dir.join(app));
            let data = data_dir_from(variable(fields[3]), variable(fields[4])).map(|dir| dir.join(app));
            assert_eq!(settings, folder(fields[5]), "line {line}: the settings folder");
            assert_eq!(data, folder(fields[6]), "line {line}: the data folder");
        }
    }

    /// Makes at `path` what a field of a `carry` case stands for.
    fn make(path: &Path, field: &str) {
        match field {
            "(none)" => {}
            "(file)" => {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"a file").unwrap();
            }
            "(empty)" => std::fs::create_dir_all(path).unwrap(),
            entries => {
                std::fs::create_dir_all(path).unwrap();
                for entry in entries.split(';') {
                    match entry.split_once('=') {
                        Some((name, text)) => {
                            let file = path.join(name);
                            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                            std::fs::write(file, text).unwrap();
                        }
                        None => std::fs::create_dir_all(path.join(entry)).unwrap(),
                    }
                }
            }
        }
    }

    /// Everything at `path`, by its path inside it with "/" ("" for `path` itself): a file with its
    /// bytes, a folder with none. Empty when nothing has the name `path`.
    fn snapshot(path: &Path) -> std::collections::BTreeMap<String, Option<Vec<u8>>> {
        fn walk(path: &Path, key: String, found: &mut std::collections::BTreeMap<String, Option<Vec<u8>>>) {
            let Ok(metadata) = std::fs::symlink_metadata(path) else { return };
            if !metadata.is_dir() {
                found.insert(key, Some(std::fs::read(path).unwrap()));
                return;
            }
            found.insert(key.clone(), None);
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name().to_string_lossy().into_owned();
                walk(&entry.path(), if key.is_empty() { name } else { format!("{key}/{name}") }, found);
            }
        }
        let mut found = std::collections::BTreeMap::new();
        walk(path, String::new(), &mut found);
        found
    }

    /// The names in a folder; none when it is missing.
    fn names_in(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .map_or_else(|_| Vec::new(), |entries| entries.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect())
    }

    #[test]
    fn carrying_the_old_folder_over() {
        for (line, fields) in cases("carry") {
            let scratch = Scratch::new(&format!("carry-{line}"));
            let (old, target) = (scratch.0.join("old"), scratch.0.join("target"));
            let new = target.join("new");
            make(&old, fields[0]);
            make(&new, fields[1]);
            let (old_before, new_before) = (snapshot(&old), snapshot(&new));
            let expected = match fields[2].split_once(' ') {
                Some(("copied", files)) => CarryOver::Copied { files: files.parse().unwrap(), left_out: Vec::new() },
                _ if fields[2] == "nothing" => CarryOver::Nothing,
                _ => panic!("line {line}: unknown outcome {}", fields[2]),
            };
            assert_eq!(carry_over(&old, &new), expected, "line {line}");
            assert_eq!(snapshot(&old), old_before, "line {line}: the old folder is left as it was");
            let copied = matches!(expected, CarryOver::Copied { .. });
            assert_eq!(snapshot(&new), if copied { old_before.clone() } else { new_before.clone() }, "line {line}: the new folder");
            let left: Vec<String> = if snapshot(&new).is_empty() { vec![] } else { vec!["new".into()] };
            assert_eq!(names_in(&target), left, "line {line}: nothing else beside the new folder");
            // The next start finds the new folder and does nothing.
            let after = snapshot(&new);
            assert_eq!(carry_over(&old, &new), CarryOver::Nothing, "line {line}: once only");
            assert_eq!((snapshot(&old), snapshot(&new)), (old_before, after), "line {line}");
        }
    }

    #[test]
    fn a_copy_that_cannot_be_made_leaves_nothing_and_says_why() {
        let scratch = Scratch::new("carry-fail");
        let old = scratch.0.join("old");
        make(&old, "settings.json={}");
        // The folder the new one goes in is a file.
        let blocked = scratch.0.join("blocked");
        std::fs::write(&blocked, b"a file").unwrap();
        let outcome = carry_over(&old, &blocked.join("new"));
        assert!(matches!(&outcome, CarryOver::Failed(reason) if !reason.is_empty()), "{outcome:?}");
        assert_eq!(std::fs::read(&blocked).unwrap(), b"a file");
        assert_eq!(snapshot(&old).len(), 2);
        let mut names = scratch.names();
        names.sort();
        assert_eq!(names, ["blocked", "old"]);
        // A new folder inside the old one would copy itself.
        let inside = old.join("deeper").join("new");
        assert!(matches!(carry_over(&old, &inside), CarryOver::Failed(_)));
        assert_eq!(names_in(&old), ["settings.json"]);
    }

    #[test]
    fn the_time_a_file_was_changed_is_kept() {
        let scratch = Scratch::new("carry-time");
        let old = scratch.0.join("old");
        make(&old, "logs/opentools.log=line");
        let file = std::fs::File::options().write(true).open(old.join("logs").join("opentools.log")).unwrap();
        let then = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        file.set_modified(then).unwrap();
        drop(file);
        let new = scratch.0.join("new");
        assert_eq!(carry_over(&old, &new), CarryOver::Copied { files: 1, left_out: vec![] });
        assert_eq!(std::fs::metadata(new.join("logs").join("opentools.log")).unwrap().modified().unwrap(), then);
    }

    #[cfg(windows)]
    #[test]
    fn a_file_in_use_is_copied_and_one_that_cannot_be_read_is_left_out() {
        use std::io::Write;
        use std::os::windows::fs::OpenOptionsExt;
        let scratch = Scratch::new("carry-in-use");
        let old = scratch.0.join("old");
        make(&old, "settings.json={};logs/app.log=start;clipboard/slot_1/objects.max=max");
        // A log another program is still writing can be read; a file it holds for itself cannot.
        let mut log = std::fs::OpenOptions::new().append(true).open(old.join("logs").join("app.log")).unwrap();
        log.write_all(b" more").unwrap();
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(old.join("clipboard").join("slot_1").join("objects.max")).unwrap();
        let new = scratch.0.join("new");
        let outcome = carry_over(&old, &new);
        drop((log, held));
        let CarryOver::Copied { files, left_out } = outcome else { panic!("{outcome:?}") };
        assert_eq!(files, 2);
        assert_eq!(left_out.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(), [Path::new("clipboard").join("slot_1").join("objects.max")]);
        assert!(!left_out[0].1.is_empty());
        assert_eq!(std::fs::read(new.join("logs").join("app.log")).unwrap(), b"start more");
        assert_eq!(names_in(&new.join("clipboard").join("slot_1")), Vec::<String>::new());
        assert_eq!(std::fs::read(old.join("clipboard").join("slot_1").join("objects.max")).unwrap(), b"max");
    }

    #[cfg(windows)]
    #[test]
    fn a_link_inside_the_old_folder_is_not_followed() {
        let scratch = Scratch::new("carry-link");
        let (old, elsewhere) = (scratch.0.join("old"), scratch.0.join("elsewhere"));
        make(&old, "settings.json={}");
        make(&elsewhere, "big.bin=far away");
        // A junction needs no administrator rights.
        let made = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(old.join("link"))
            .arg(&elsewhere)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(made.success());
        let new = scratch.0.join("new");
        let outcome = carry_over(&old, &new);
        let CarryOver::Copied { files: 1, left_out } = outcome else { panic!("{outcome:?}") };
        assert_eq!(left_out.iter().map(|(path, _)| path.clone()).collect::<Vec<_>>(), [PathBuf::from("link")]);
        assert_eq!(names_in(&new), ["settings.json"]);
        assert_eq!(names_in(&elsewhere), ["big.bin"]);
    }
}
