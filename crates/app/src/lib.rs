#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod commands;
#[allow(clippy::unwrap_used)]
pub mod fakes;
pub mod services;
