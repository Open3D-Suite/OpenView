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
- `tools/package.py`: the packager. `build.rs`: the exe's icon and file properties.

## Build and check

- Build: `cargo build --release --locked`.
- Checks, all of which must pass:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --locked` (84 tests; none needs the Internet)
  - `ruff format --check .` and `ruff check .`, for `tools/package.py`
- `cargo test --locked --test live -- --ignored` runs the 6 tests that need the network (ignored otherwise).
- `.editorconfig`, `.gitattributes`, `rustfmt.toml`, `clippy.toml` and `ruff.toml` are the same in every
  Open3D repo. Do not change them here. No formatter touches `ui/`.

## Version and release

- The version is in `Cargo.toml` and nowhere else by hand, except the `**Version X.Y.Z**` line of the
  README and the CHANGELOG's `## X.Y.Z` heading. The packager checks both; `## In development` also passes.
- `python -I tools/package.py` checks the licences of the dependencies, runs the tests, builds, and writes
  `dist/openview-<version>-windows-x64.zip` and `dist/SHA256SUMS.txt`. It refuses an exe that still holds a
  path of the computer that built it, or that does not answer `OpenView <version>` to `--version`.
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
