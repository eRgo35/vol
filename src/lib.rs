//! vol: a terminal mixer for PipeWire.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod action;
pub mod app;
pub mod backend;
pub mod cli;
pub mod event;
pub mod keymap;
pub mod model;
pub mod tabs;
pub mod theme;
pub mod ui;
pub mod ui_state;
