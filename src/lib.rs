// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenView: downloads 360 panoramas and photos from map applications at full resolution, with a
//! viewer, crops and HDRI. Part of the Open3D suite.
//!
//! Based on the original OpenView by Daniel Castellanos (MIT, see NOTICE): its interface is used
//! as it is, and its download, crop and HDRI methods are ported to Rust.

pub mod cli;
pub mod extras;
pub mod geom;
pub mod hdri;
pub mod links;
// The language shared by every Open3D app: a copy of a common file. Its one call to Windows, for
// the language of Windows, allows `unsafe` by name.
pub mod open3d_settings;
pub mod pano;
pub mod service;
pub mod settings;
pub mod text;
// `unsafe` is allowed in this module: it gives the file dialogs the app's window as a raw Windows handle.
#[allow(unsafe_code)]
pub mod web;
// `unsafe` is allowed in this module: it calls Windows to look for a monitor and to show a message box.
#[allow(unsafe_code)]
pub mod window;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
