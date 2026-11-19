# OpenView

Downloads 360 panoramas and photos from map applications at full resolution, with a 360 viewer, crops and
a converter to HDRI. One Windows program, written in Rust: a tao + wry (WebView2) window over a local server.

This file holds facts about this repo only, each one checkable in the code. Update it in the same commit
as a change that makes it old.

## Layout

- `src/main.rs`: no arguments or `--window` opens the window (`src/window.rs`); the rest is `src/cli.rs`.
- `src/web.rs`: the local server and its API. `ui/index.html`: the whole page, with its texts in
  `ui/i18n/en.json` and `es.json`; the three files are built into the exe.
- `src/service.rs`: every request to the map service. `src/links.rs`: reads a pasted link, with no network.
- `src/pano.rs`: downloads, tiles, crops, saving. `src/geom.rs`: the geometry of 360 images. `src/extras.rs`:
  north, ground patch, partial photos. `src/hdri.rs`: the HDRI converter.
- `src/text.rs`: the program's own messages. `src/settings.rs`: the settings, in `%APPDATA%\OpenView`.
- `build.rs`: the exe's icon and file properties.

## Build and check

- Build: `cargo build --release --locked`.
- Checks: `tools\check.ps1` runs them all, and all must pass. It needs Python 3.11 or newer and the
  ruff that `ruff.toml` names. First what `tools/repo.py` has built in, then what `tools/repo.toml` lists:
  - the version agrees in its three places; every source has its licence line; `Cargo.toml` has the
    common lints; no file holds a path of somebody's computer
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --locked` (84 tests; none needs the Internet)
  - `ruff format --check .` and `ruff check .`, for the Python in `tools/`
  - only with `-Full`: `cargo test --locked --test live -- --ignored` (6 tests that need the network)
- CI: `.github/workflows/check.yml` runs `tools\check.ps1`, without `-Full`, on every push and pull request
  to `main`, on GitHub's Windows runner with Python 3.14.
- `.editorconfig`, `.gitattributes`, `rustfmt.toml`, `clippy.toml`, `ruff.toml`, `LICENSE`,
  `tools/check.ps1`, `tools/repo.py`, `tools/repo_package.py` and `tools/repo_release.py` are the same in
  every Open3D repo. Do not change them here. No formatter touches `ui/`.

## Version and release

- The version is in `Cargo.toml`; `python -I tools/repo.py version set X.Y.Z` also writes it into
  `Cargo.lock` and the README's `**Version X.Y.Z**` line; `tools\check.ps1` fails if they disagree.
- `python -I tools/repo.py package` makes the release package as the `[package]` table of
  `tools/repo.toml` says: the version and its `## X.Y.Z` section in the CHANGELOG, the quick checks,
  `cargo build --release --locked` without this computer's folders in the exe, the licences of the
  dependencies in `THIRD-PARTY-LICENSES.txt`, then `dist/openview-<version>-windows-x64.zip` (`OpenView.exe`,
  README, LICENSE, NOTICE, CHANGELOG, `LICENSES/*.txt`) and `dist/SHA256SUMS.txt`. It refuses an exe that
  holds a path of the computer that built it, or whose file version or `--version` is not the version.
- `python -I tools/repo.py release X.Y.Z` turns `## In development` into `## X.Y.Z — <date>` (without that
  section it stops), sets the version, commits `Release X.Y.Z`, packages, tags `vX.Y.Z` and prints the
  pushes to approve; run again once they are done, it makes the GitHub release as a draft. It never
  pushes. `--dry-run` tries it all and writes only to `dist/`.
- The Open3D launcher needs `OpenView.exe` at the root of the zip, with the release's version as its file
  version; `build.rs` takes it from `Cargo.toml`.

## Rules of this code

- The page reaches the program only through the local server, on `127.0.0.1` and a free port. Every POST
  and `/api/state` need the run's token in `X-Token`, and are refused when `Origin` is another site.
- Every text of the page is a key of both files in `ui/i18n/`, written `{{key}}` in the markup or `T.key`
  in the script. A test fails when the files differ in keys or the page uses a key that has no text.
- Requests to the map service and the parsing of its JSON answers are in `src/service.rs`, and the forms
  of its links in `src/links.rs`: the service is not documented and may change without notice.
- `unsafe` is denied. It is allowed by name, with its reason, in four places that call Windows or pass a
  window handle: the `text`, `web` and `window` modules and `attach_console` in `src/main.rs`.
- The code comes from the original OpenView by Daniel Castellanos (MIT): the page is its interface and the
  server keeps its routes and answers. `NOTICE` and `LICENSES/` stay in the repo and go into every zip.

## Traps

- The exe is a window program: the command line attaches to the parent console. Debug builds enable devtools.
- Without WebView2 the page opens in the browser; the program ends when `/api/ping` stops and no job runs.
- `Cargo.lock` holds the version too: after changing it, `--locked` commands fail until the lock is updated.
- Debug builds and tests are optimised too (`[profile.dev]`): the image code is too slow otherwise.
- Keys of `ui/i18n/` ending in `_html` go into the page with their markup; the others are escaped.
- The page's 360 viewer looks along -z and `src/geom.rs` along +z: `viewer_matrix` adapts the levelling.
