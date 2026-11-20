// SPDX-License-Identifier: GPL-3.0-or-later
//! What the app remembers between runs: the download options, the theme, the language and the
//! window's place. Kept in `settings.json` in `%APPDATA%\Open3D\OpenView` (`folders.rs`), with the
//! same keys the interface uses. The language's source is the one every Open3D app shares
//! (`open3d_settings`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::open3d_settings::{self, Language};
use crate::pano::{Format, Options};

/// Where the window was, in screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPlace {
    pub w: u32,
    pub h: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub max: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub outdir: PathBuf,
    pub fmt: Format,
    pub north: bool,
    pub json: bool,
    /// Cover the ground under the camera.
    pub nadir: bool,
    /// Complete partial 360 photos to 2:1.
    pub fill: bool,
    /// Level the horizon.
    pub level: bool,
    /// "", "light" or "dark" ("" follows Windows).
    pub theme: String,
    /// "", "en" or "es" ("" follows Windows): the language shown. The language every Open3D app
    /// shares is the source: at start this takes its value ([`Settings::take_shared_language`]),
    /// and a language chosen on the page is written there ([`Settings::share_language`]).
    pub lang: String,
    /// The language chosen here before has been handed to the shared file (or there was nothing
    /// to hand over), so it is never carried over again.
    pub language_shared: bool,
    pub window: Option<WindowPlace>,
}

impl Default for Settings {
    fn default() -> Self {
        let options = Options::default();
        Settings {
            outdir: default_out_dir(),
            fmt: options.format,
            north: options.north,
            json: options.json,
            nadir: options.cover_ground,
            fill: options.complete,
            level: options.level,
            theme: String::new(),
            lang: String::new(),
            language_shared: false,
            window: None,
        }
    }
}

fn default_out_dir() -> PathBuf {
    match std::env::var_os("USERPROFILE") {
        Some(home) => PathBuf::from(home).join("Pictures").join("OpenView"),
        None => PathBuf::from("."),
    }
}

impl Settings {
    /// The settings saved in `file` (`folders::Folders::settings_file` in the app), or the defaults
    /// when there are none or they cannot be read.
    pub fn load_from(file: &Path) -> Settings {
        fs::read_to_string(file).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    /// Keeps the settings in `file`.
    pub fn save_to(&self, file: &Path) {
        if let Some(dir) = file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = fs::write(file, text);
        }
    }

    /// Takes the language every Open3D app shares in at start, from `settings.json` in `dir`
    /// (D-051): its preference becomes `lang`. With none, a language the person chose here
    /// before (`en` or `es`; "" is no choice) is written there once (D-048); otherwise `lang` is
    /// "", and Windows decides (D-052). Nothing is written when there is nothing to carry over. A
    /// write that fails keeps `lang`, is tried again at the next start, and its error is returned.
    pub fn take_shared_language(&mut self, dir: Option<&Path>) -> Result<(), String> {
        if let Some(shared) = dir.and_then(open3d_settings::read_language) {
            self.lang = shared.code().to_string();
            self.language_shared = true;
            return Ok(());
        }
        if !self.language_shared
            && let Some(own) = Language::from_code(&self.lang)
        {
            let dir = dir.ok_or("no folder for the language shared by the Open3D apps")?;
            open3d_settings::write_language(dir, Some(own))?;
            self.language_shared = true;
            return Ok(());
        }
        self.language_shared = true;
        self.lang = String::new();
        Ok(())
    }

    /// Writes `lang`, just chosen on the page, to `settings.json` in `dir` for every Open3D app
    /// (D-056): "en" or "es" is the preference, "" takes it out so Windows decides (D-059). They
    /// take it the next time they start (D-057). The error says what failed.
    pub fn share_language(&mut self, dir: Option<&Path>) -> Result<(), String> {
        let dir = dir.ok_or("no folder for the language shared by the Open3D apps")?;
        open3d_settings::write_language(dir, Language::from_code(&self.lang))?;
        self.language_shared = true;
        Ok(())
    }

    /// The download options these settings stand for.
    pub fn options(&self) -> Options {
        Options {
            out_dir: self.outdir.clone(),
            format: self.fmt,
            north: self.north,
            level: self.level,
            cover_ground: self.nadir,
            complete: self.fill,
            json: self.json,
            max_width: None,
        }
    }

    /// Applies what the page sends: only known keys, only with the right type and value. The
    /// window's place, the folder (chosen in a dialog) and `language_shared` are not the page's
    /// to set.
    pub fn apply_patch(&mut self, patch: &Value) {
        let Some(patch) = patch.as_object() else { return };
        let mut current = serde_json::to_value(&*self).unwrap_or(Value::Null);
        let Some(fields) = current.as_object_mut() else { return };
        for (key, value) in patch {
            if matches!(key.as_str(), "window" | "outdir" | "language_shared") {
                continue;
            }
            let Some(slot) = fields.get_mut(key) else { continue };
            let same_type = matches!((&*slot, value), (Value::Bool(_), Value::Bool(_)) | (Value::String(_), Value::String(_)));
            let allowed = match key.as_str() {
                "fmt" => matches!(value.as_str(), Some("jpg" | "png" | "tif")),
                "theme" => matches!(value.as_str(), Some("" | "light" | "dark")),
                "lang" => matches!(value.as_str(), Some("" | "en" | "es")),
                _ => true,
            };
            if same_type && allowed {
                *slot = value.clone();
            }
        }
        if let Ok(updated) = serde_json::from_value(current) {
            *self = updated;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_survive_a_round_trip() {
        let settings = Settings {
            fmt: Format::Tif,
            north: true,
            window: Some(WindowPlace { w: 900, h: 700, x: Some(10), y: None, max: false }),
            ..Default::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        assert!(text.contains(r#""fmt":"tif""#), "{text}");
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), settings);
    }

    #[test]
    fn missing_and_unknown_fields_fall_back_to_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"north": true, "from_the_future": 1}"#).unwrap();
        assert!(settings.north && settings.level && settings.fill && !settings.nadir);
        assert_eq!((settings.fmt, settings.theme.as_str()), (Format::Jpg, ""));
    }

    #[test]
    fn the_page_can_only_change_what_it_should() {
        let mut settings = Settings::default();
        let before = settings.clone();
        settings.apply_patch(&json!({
            "north": true, "fmt": "png", "theme": "dark", "lang": "es",
            "level": "yes", "json": 1, "outdir": "C:/elsewhere", "window": {"w": 1, "h": 1, "max": true},
            "language_shared": true, "unknown": true
        }));
        assert!(settings.north && settings.level && !settings.json && !settings.language_shared);
        assert_eq!((settings.fmt, settings.theme.as_str(), settings.lang.as_str()), (Format::Png, "dark", "es"));
        assert_eq!((settings.outdir.clone(), settings.window), (before.outdir, None));
        settings.apply_patch(&json!({"fmt": "bmp", "theme": "pink", "lang": "fr"}));
        assert_eq!((settings.fmt, settings.theme.as_str(), settings.lang.as_str()), (Format::Png, "dark", "es"));
    }

    #[test]
    fn options_follow_the_settings() {
        let settings = Settings { nadir: true, level: false, ..Default::default() };
        let options = settings.options();
        assert!(options.cover_ground && !options.level && options.max_width.is_none());
    }

    /// A folder of its own for the shared `settings.json`, never the real `%APPDATA%\Open3D`.
    struct Shared(PathBuf);

    impl Shared {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("openview-shared-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            let _ = fs::remove_file(&dir);
            Self(dir)
        }
        fn holding(name: &str, json: &str) -> Self {
            let shared = Self::new(name);
            fs::create_dir_all(&shared.0).unwrap();
            fs::write(shared.0.join(open3d_settings::FILE_NAME), json).unwrap();
            shared
        }
        fn file(&self) -> Option<Value> {
            serde_json::from_slice(&fs::read(self.0.join(open3d_settings::FILE_NAME)).ok()?).ok()
        }
    }

    impl Drop for Shared {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn settings_are_read_from_the_file_they_are_kept_in() {
        let folder = Shared::new("file");
        let file = folder.0.join("OpenView").join("settings.json");
        assert_eq!(Settings::load_from(&file), Settings::default());
        let settings = Settings { theme: "dark".into(), fmt: Format::Png, ..Default::default() };
        settings.save_to(&file);
        assert_eq!(Settings::load_from(&file), settings);
    }

    fn with(lang: &str, language_shared: bool) -> Settings {
        Settings { lang: lang.into(), language_shared, ..Default::default() }
    }

    #[test]
    fn the_shared_language_wins_over_openviews_own() {
        let json = r#"{"language":"es","theme":"dark"}"#;
        let shared = Shared::holding("wins", json);
        for own in ["", "en", "es"] {
            let mut settings = with(own, false);
            assert_eq!(settings.take_shared_language(Some(&shared.0)), Ok(()));
            assert_eq!((settings.lang.as_str(), settings.language_shared), ("es", true), "{own:?}");
        }
        let file = fs::read_to_string(shared.0.join(open3d_settings::FILE_NAME)).unwrap();
        assert_eq!(file, json, "reading writes nothing");
    }

    #[test]
    fn with_nothing_chosen_windows_decides_and_nothing_is_written() {
        let shared = Shared::new("nothing");
        let mut settings = with("", false);
        assert_eq!(settings.take_shared_language(Some(&shared.0)), Ok(()));
        assert_eq!((settings.lang.as_str(), settings.language_shared), ("", true));
        assert!(!shared.0.exists(), "nothing is written");
        // A file that holds no preference stays as it was.
        let shared = Shared::holding("no-preference", r#"{"language":"fr"}"#);
        let mut settings = with("", false);
        assert_eq!(settings.take_shared_language(Some(&shared.0)), Ok(()));
        assert_eq!(settings.lang, "");
        assert_eq!(shared.file(), Some(json!({"language": "fr"})));
    }

    #[test]
    fn a_language_chosen_here_is_carried_over_once() {
        for (own, language) in [("en", Language::English), ("es", Language::Spanish)] {
            let shared = Shared::new(&format!("carried-{own}"));
            let mut settings = with(own, false);
            assert_eq!(settings.take_shared_language(Some(&shared.0)), Ok(()));
            assert_eq!((settings.lang.as_str(), settings.language_shared), (own, true));
            assert_eq!(open3d_settings::read_language(&shared.0), Some(language));
            // Taken out later (Auto in the launcher): OpenView's copy is not put back.
            assert_eq!(open3d_settings::write_language(&shared.0, None), Ok(()));
            assert_eq!(settings.take_shared_language(Some(&shared.0)), Ok(()));
            assert_eq!((settings.lang.as_str(), settings.language_shared), ("", true));
            assert_eq!(shared.file(), Some(json!({})));
        }
    }

    #[test]
    fn a_carry_over_that_fails_keeps_the_language_and_is_tried_again() {
        let shared = Shared::new("blocked");
        // The folder is a file: nothing can be written in it.
        fs::write(&shared.0, b"in the way").unwrap();
        let mut settings = with("es", false);
        assert!(settings.take_shared_language(Some(&shared.0)).is_err());
        assert_eq!((settings.lang.as_str(), settings.language_shared), ("es", false));
        assert!(with("es", false).take_shared_language(None).is_err());
        fs::remove_file(&shared.0).unwrap();
        assert_eq!(settings.take_shared_language(Some(&shared.0)), Ok(()));
        assert_eq!((settings.lang.as_str(), settings.language_shared), ("es", true));
        assert_eq!(open3d_settings::read_language(&shared.0), Some(Language::Spanish));
    }

    #[test]
    fn settings_saved_before_have_not_handed_their_language_over() {
        let settings: Settings = serde_json::from_str(r#"{"lang":"es"}"#).unwrap();
        assert_eq!((settings.lang.as_str(), settings.language_shared), ("es", false));
    }

    #[test]
    fn a_language_chosen_on_the_page_is_shared_and_follow_windows_takes_it_out() {
        let shared = Shared::holding("chosen", r#"{"theme":"dark"}"#);
        let mut settings = with("es", false);
        assert_eq!(settings.share_language(Some(&shared.0)), Ok(()));
        assert!(settings.language_shared);
        assert_eq!(shared.file(), Some(json!({"language": "es", "theme": "dark"})));
        settings.lang = String::new();
        assert_eq!(settings.share_language(Some(&shared.0)), Ok(()));
        assert_eq!(shared.file(), Some(json!({"theme": "dark"})));
        // A write that fails says why.
        let blocked = Shared::new("chosen-blocked");
        fs::write(&blocked.0, b"in the way").unwrap();
        let mut settings = with("en", false);
        assert!(settings.share_language(Some(&blocked.0)).is_err());
        assert!(!settings.language_shared);
        assert!(settings.share_language(None).is_err());
    }
}
