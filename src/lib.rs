//! Clay Form: a UI-independent sculpt document with desktop input/rendering adapters.
mod action;
mod app;
mod background_job;
mod brush_icon;
pub mod camera;
mod command;
pub mod diagnostics;
pub mod document;
mod history;
mod lighting;
mod mesh_job;
pub mod persistence;
mod preferences;
mod recovery;
pub mod reference;
pub mod sculpt;
mod tablet;
mod touch;
mod ui;
mod viewport;
pub use app::App;

pub use tablet::configure_native_input;
