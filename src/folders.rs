// SPDX-License-Identifier: GPL-3.0-or-later
//! Where OpenView keeps its files: its `settings.json` in `%APPDATA%\Open3D\OpenView`, and the data of
//! its built-in browser (WebView2) in `%LOCALAPPDATA%\Open3D\OpenView\webview`, or in `OpenView` inside
//! the folders that `OPEN3D_SETTINGS_DIR` and `OPEN3D_DATA_DIR` name (D-042, D-044).
//!
//! At start, before the settings are read and before WebView2 writes anything, [`prepare`] copies the
//! `settings.json` of an OpenView from before, in `%APPDATA%\OpenView`, into the new folder, once, and
//! leaves it as it was (D-048, D-049). Only that file: the browser's data beside it, in
//! `%APPDATA%\OpenView\webview`, is not copied, and WebView2 makes it again in the new folder (D-049).
//! When the copy fails, that run uses the old file, as before, and the next start tries again.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::open3d_settings::{self, CarryOver};

/// The app's name in the Open3D folders.
pub const APP: &str = "OpenView";
/// The file of OpenView's settings, in its settings folder.
pub const SETTINGS_FILE: &str = "settings.json";
/// The folder of the built-in browser's data, in OpenView's data folder.
pub const WEBVIEW: &str = "webview";

/// The folders this run uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folders {
    /// OpenView's settings; `None` keeps them in memory only.
    pub settings_file: Option<PathBuf>,
    /// The data of the built-in browser; `None` leaves it to WebView2.
    pub webview: Option<PathBuf>,
    /// Why the settings of an OpenView from before could not be copied, to tell the person.
    pub problem: Option<String>,
}

/// The folders of this run, once the old settings have been carried over. Call it first, before the
/// settings are read and before WebView2 starts.
pub fn prepare() -> Folders {
    folders(old_folder(), open3d_settings::app_settings_dir(APP), open3d_settings::app_data_dir(APP))
}

/// [`prepare`] with these folders: OpenView's old one, and its new settings and data folders.
fn folders(old: Option<PathBuf>, settings: Option<PathBuf>, data: Option<PathBuf>) -> Folders {
    let (settings_file, problem) = carry(old.map(|dir| dir.join(SETTINGS_FILE)), settings.map(|dir| dir.join(SETTINGS_FILE)));
    Folders { settings_file, webview: data.map(|dir| dir.join(WEBVIEW)), problem }
}

/// The folder OpenView used before: `%APPDATA%\OpenView`.
fn old_folder() -> Option<PathBuf> {
    std::env::var_os("APPDATA").filter(|dir| !dir.is_empty()).map(|dir| PathBuf::from(dir).join(APP))
}

/// Carries the file `old` over to `new` and gives the file to use, with why the copy failed, if it
/// did: `new`, or `old` when the copy failed, so that this run does not make `new` and lose the copy.
fn carry(old: Option<PathBuf>, new: Option<PathBuf>) -> (Option<PathBuf>, Option<String>) {
    let (Some(old), Some(new)) = (old.as_deref(), new.as_deref()) else { return (new.or(old), None) };
    match carry_file(old, new) {
        CarryOver::Nothing | CarryOver::Copied { .. } => (Some(new.to_path_buf()), None),
        CarryOver::Failed(reason) => (Some(old.to_path_buf()), Some(reason)),
    }
}

/// Copies the file `old` to `new` with the rules [`open3d_settings::carry_over`] keeps for a folder,
/// for OpenView, which copies only its settings (D-049). Only when nothing has the name `new` and `old`
/// is a file. The copy, with the time `old` was last changed, is written into a new file beside `new`,
/// which takes the name `new` once it is complete and only while nothing has that name: an existing
/// `new` is never replaced, and a copy cut half way is never taken for a finished one. `old` is opened
/// as other programs may still use it, and is never changed. When the copy cannot be made, nothing is
/// put in place and the next start tries again. It never panics.
pub fn carry_file(old: &Path, new: &Path) -> CarryOver {
    if exists(new) || !old.is_file() {
        return CarryOver::Nothing;
    }
    let (Some(dir), Some(name)) = (new.parent(), new.file_name()) else {
        return CarryOver::Failed(format!("{} cannot be a file of its own", new.display()));
    };
    let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
    let (bytes, modified) = match read(old) {
        Ok(read) => read,
        Err(e) => return CarryOver::Failed(format!("couldn't read {} ({e})", old.display())),
    };
    let temporary = match std::fs::create_dir_all(dir).and_then(|()| write_beside(dir, name, &bytes, modified)) {
        Ok(temporary) => temporary,
        Err(e) => return CarryOver::Failed(format!("couldn't write a file in {} ({e})", dir.display())),
    };
    let placed = put_in_place(&temporary, new);
    // After a hard link it is a second name of the copy, and after a failure the unused copy: either
    // way it goes. After a rename it is gone already.
    let _ = std::fs::remove_file(&temporary);
    match placed {
        Ok(true) => CarryOver::Copied { files: 1, left_out: Vec::new() },
        // Another start of OpenView put its own copy in place meanwhile.
        Ok(false) => CarryOver::Nothing,
        Err(reason) => CarryOver::Failed(reason),
    }
}

/// Whether something has the name `path`: a file, a folder or a link. When that cannot be told, it
/// counts as there, so that it is never touched.
fn exists(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(e) => !matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory),
    }
}

/// The bytes of the file `path` and the time it was last changed. It is opened as other programs may
/// still use it.
fn read(path: &Path) -> std::io::Result<(Vec<u8>, Option<SystemTime>)> {
    let mut file = std::fs::File::open(path)?;
    let modified = file.metadata().and_then(|metadata| metadata.modified()).ok();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok((bytes, modified))
}

/// A new file of its own in `dir`, named after `name`, holding `bytes` and changed last at `modified`.
fn write_beside(dir: &Path, name: &std::ffi::OsStr, bytes: &[u8], modified: Option<SystemTime>) -> std::io::Result<PathBuf> {
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |time| time.as_nanos());
    for attempt in 0..100 {
        let mut file_name = name.to_os_string();
        file_name.push(format!(".carry-{}-{stamp}-{attempt}.tmp", std::process::id()));
        let path = dir.join(file_name);
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let written = file.write_all(bytes).and_then(|()| {
            if let Some(modified) = modified {
                file.set_modified(modified)?;
            }
            file.sync_all()
        });
        drop(file);
        return match written {
            Ok(()) => Ok(path),
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Err(e)
            }
        };
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "every name tried is taken"))
}

/// Gives the finished copy the name `new`: `Ok(false)` when something has that name by then. A hard
/// link never takes the place of what is there; where the disk has no hard links, a rename, once
/// nothing has the name. It tries a few times: another program, such as an antivirus, may hold the
/// copy for a moment.
fn put_in_place(temporary: &Path, new: &Path) -> Result<bool, String> {
    let mut last = String::new();
    for attempt in 0..10 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if exists(new) {
            return Ok(false);
        }
        let placed = std::fs::hard_link(temporary, new)
            .or_else(|e| if e.kind() == std::io::ErrorKind::AlreadyExists || exists(new) { Err(e) } else { std::fs::rename(temporary, new) });
        match placed {
            Ok(()) => return Ok(true),
            Err(_) if exists(new) => return Ok(false),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("couldn't put {} in place of {} ({last})", temporary.display(), new.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A folder of its own, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("openview-folders-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The settings of an OpenView from before, as a person who uses it has them.
    const OLD_SETTINGS: &str = r#"{"outdir": "D:\\Panoramas", "fmt": "png", "theme": "dark", "lang": "es", "language_shared": true}"#;

    /// An OpenView from before: `Roaming\OpenView` in `root`, with its settings and its browser's data.
    fn old_openview(root: &Path) -> PathBuf {
        let old = root.join("Roaming").join(APP);
        for (name, content) in [
            (SETTINGS_FILE, OLD_SETTINGS.as_bytes()),
            ("webview/EBWebView/Local State", b"{}".as_slice()),
            ("webview/EBWebView/Default/Cache/data_0", &[7; 4096]),
        ] {
            let file = old.join(name);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
        }
        old
    }

    /// Everything at `path`, by its path inside it with "/": a file with its bytes, a folder with none.
    fn snapshot(path: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
        fn walk(path: &Path, key: String, found: &mut BTreeMap<String, Option<Vec<u8>>>) {
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
        let mut found = BTreeMap::new();
        walk(path, String::new(), &mut found);
        found
    }

    /// The names in a folder, sorted; none when it is missing.
    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map_or_else(|_| Vec::new(), |entries| entries.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect());
        names.sort();
        names
    }

    #[test]
    fn only_the_settings_are_copied_once_and_the_new_folders_used() {
        let scratch = Scratch::new("copy");
        let old = old_openview(&scratch.0);
        let file = old.join(SETTINGS_FILE);
        let then = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        std::fs::File::options().write(true).open(&file).unwrap().set_modified(then).unwrap();
        let before = snapshot(&old);
        let (settings, data) = (scratch.0.join("Open3D").join(APP), scratch.0.join("Local").join("Open3D").join(APP));
        let found = folders(Some(old.clone()), Some(settings.clone()), Some(data.clone()));
        let expected = Folders { settings_file: Some(settings.join(SETTINGS_FILE)), webview: Some(data.join(WEBVIEW)), problem: None };
        assert_eq!(found, expected);
        assert_eq!(std::fs::read_to_string(settings.join(SETTINGS_FILE)).unwrap(), OLD_SETTINGS);
        assert_eq!(std::fs::metadata(settings.join(SETTINGS_FILE)).unwrap().modified().unwrap(), then);
        // Only the settings, and nothing beside them; the browser's data is made again, by WebView2.
        assert_eq!(names_in(&settings), [SETTINGS_FILE]);
        assert_eq!(names_in(settings.parent().unwrap()), [APP]);
        assert!(!data.exists(), "nothing is written in the data folder");
        assert_eq!(snapshot(&old), before, "the old folder is left as it was");
        // The next start finds the new settings and uses them as they are.
        std::fs::write(settings.join(SETTINGS_FILE), "{}").unwrap();
        assert_eq!(folders(Some(old.clone()), Some(settings.clone()), Some(data.clone())), expected);
        assert_eq!(std::fs::read(settings.join(SETTINGS_FILE)).unwrap(), b"{}");
        assert_eq!(snapshot(&old), before);
    }

    #[test]
    fn what_has_the_new_name_is_never_replaced() {
        let scratch = Scratch::new("there");
        let old = old_openview(&scratch.0);
        let new = scratch.0.join("new").join(SETTINGS_FILE);
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::write(&new, "{\"theme\": \"light\"}").unwrap();
        assert_eq!(carry_file(&old.join(SETTINGS_FILE), &new), CarryOver::Nothing);
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "{\"theme\": \"light\"}");
        // A folder with that name is not touched either.
        let folder = scratch.0.join("folder").join(SETTINGS_FILE);
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(carry_file(&old.join(SETTINGS_FILE), &folder), CarryOver::Nothing);
        assert_eq!(names_in(&folder), Vec::<String>::new());
        assert_eq!(names_in(folder.parent().unwrap()), [SETTINGS_FILE]);
    }

    #[test]
    fn without_old_settings_nothing_is_copied_or_made() {
        let scratch = Scratch::new("none");
        // An OpenView from before that never saved its settings, with only its browser's data.
        let old = old_openview(&scratch.0);
        std::fs::remove_file(old.join(SETTINGS_FILE)).unwrap();
        let settings = scratch.0.join("Open3D").join(APP);
        let found = folders(Some(old.clone()), Some(settings.clone()), None);
        assert_eq!(found, Folders { settings_file: Some(settings.join(SETTINGS_FILE)), webview: None, problem: None });
        assert!(!scratch.0.join("Open3D").exists(), "no folder is made");
        // A folder in place of the old settings is not a file to copy.
        std::fs::create_dir(old.join(SETTINGS_FILE)).unwrap();
        assert_eq!(carry_file(&old.join(SETTINGS_FILE), &settings.join(SETTINGS_FILE)), CarryOver::Nothing);
        assert!(!scratch.0.join("Open3D").exists());
    }

    #[test]
    fn without_one_of_the_folders_nothing_is_copied() {
        let scratch = Scratch::new("missing");
        let old = old_openview(&scratch.0).join(SETTINGS_FILE);
        let new = scratch.0.join("new").join(SETTINGS_FILE);
        assert_eq!(carry(Some(old.clone()), None), (Some(old), None));
        assert_eq!(carry(None, Some(new.clone())), (Some(new.clone()), None));
        assert!(!new.exists());
        assert_eq!(carry(None, None), (None, None));
        assert_eq!(folders(None, None, None), Folders::default());
    }

    #[test]
    fn a_copy_that_fails_leaves_the_old_file_in_use_and_is_tried_again() {
        let scratch = Scratch::new("fail");
        let old = old_openview(&scratch.0);
        let before = snapshot(&old);
        // The folder the new settings go in is a file.
        let blocked = scratch.0.join("Open3D");
        std::fs::write(&blocked, b"a file").unwrap();
        let found = folders(Some(old.clone()), Some(blocked.join(APP)), Some(scratch.0.join("Local")));
        assert_eq!(found.settings_file, Some(old.join(SETTINGS_FILE)));
        assert_eq!(found.webview, Some(scratch.0.join("Local").join(WEBVIEW)));
        assert!(found.problem.is_some_and(|problem| !problem.is_empty()));
        assert_eq!(std::fs::read(&blocked).unwrap(), b"a file");
        assert_eq!(snapshot(&old), before);
        // The next start, with the way clear, copies them.
        std::fs::remove_file(&blocked).unwrap();
        let again = folders(Some(old.clone()), Some(blocked.join(APP)), None);
        assert_eq!((again.settings_file, again.problem), (Some(blocked.join(APP).join(SETTINGS_FILE)), None));
        assert_eq!(std::fs::read_to_string(blocked.join(APP).join(SETTINGS_FILE)).unwrap(), OLD_SETTINGS);
    }

    #[cfg(windows)]
    #[test]
    fn settings_in_use_are_copied_and_settings_held_for_another_program_are_not() {
        use std::os::windows::fs::OpenOptionsExt;
        let scratch = Scratch::new("in-use");
        let old = old_openview(&scratch.0).join(SETTINGS_FILE);
        // An older OpenView still running has them open to write them.
        let open = std::fs::OpenOptions::new().append(true).open(&old).unwrap();
        let new = scratch.0.join("new").join(SETTINGS_FILE);
        assert_eq!(carry_file(&old, &new), CarryOver::Copied { files: 1, left_out: Vec::new() });
        assert_eq!(std::fs::read_to_string(&new).unwrap(), OLD_SETTINGS);
        drop(open);
        // A program that holds them for itself: nothing is put in place, and the old file is used.
        let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&old).unwrap();
        let other = scratch.0.join("other").join(SETTINGS_FILE);
        let (file, problem) = carry(Some(old.clone()), Some(other.clone()));
        drop(held);
        assert_eq!(file, Some(old.clone()));
        assert!(problem.is_some_and(|problem| problem.contains(&old.display().to_string())));
        assert_eq!(names_in(other.parent().unwrap()), Vec::<String>::new(), "nothing is left beside the new file");
    }
}
