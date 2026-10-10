//! Clay Form: a UI-independent sculpt document with desktop input/rendering adapters.
mod app;
pub mod camera;
mod command;
pub mod document;
mod history;
pub mod persistence;
mod preferences;
pub mod sculpt;
mod tablet;
mod ui;
mod viewport;
pub use app::App;
