// SPDX-License-Identifier: GPL-3.0-or-later
//! Where OpenView keeps its files when Windows gives it neither `USERPROFILE` nor `LOCALAPPDATA`, and
//! `OPEN3D_DATA_DIR` is not set. Each test runs itself again without those variables, as a program
//! started that way would run, and that run checks the folders. Its settings stay in scratch folders,
//! so no real settings are read or written.

use std::path::PathBuf;
use std::process::Command;

/// Set only in a run without the variables: the profile folder of the run that started it.
const PROFILE: &str = "OPENVIEW_TEST_PROFILE";

/// In the run without the variables, the profile folder of the run that started it. Otherwise it runs
/// the test `name` again without them, fails when that run fails, and gives `None`.
fn profile_or_run_again(name: &str) -> Option<PathBuf> {
    if let Some(profile) = std::env::var_os(PROFILE) {
        return Some(PathBuf::from(profile));
    }
    let profile = std::env::var_os("USERPROFILE").expect("the test compares with this run's USERPROFILE");
    let scratch = std::env::temp_dir().join(format!("openview-profile-{}-{name}", std::process::id()));
    let run = Command::new(std::env::current_exe().unwrap())
        .args([name, "--exact", "--test-threads=1"])
        .env_remove("USERPROFILE")
        .env_remove("LOCALAPPDATA")
        .env_remove("OPEN3D_DATA_DIR")
        .env("APPDATA", scratch.join("Roaming"))
        .env("OPEN3D_SETTINGS_DIR", scratch.join("Open3D"))
        .env(PROFILE, &profile)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&scratch);
    let said = format!("{}{}", String::from_utf8_lossy(&run.stdout), String::from_utf8_lossy(&run.stderr));
    assert!(run.status.success() && said.contains("1 passed"), "{said}");
    None
}

#[test]
fn without_the_profile_variables_the_window_keeps_its_data_in_the_profile() {
    let Some(profile) = profile_or_run_again("without_the_profile_variables_the_window_keeps_its_data_in_the_profile") else { return };
    // Never beside the exe, where WebView2 cannot write once OpenView is in Program Files.
    let webview = openview::folders::prepare().webview;
    assert_eq!(webview, Some(profile.join("AppData").join("Local").join("Open3D").join("OpenView").join("webview")));
}
