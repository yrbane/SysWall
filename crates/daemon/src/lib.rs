//! Bibliothèque du daemon SysWall : exposée pour les tests d'intégration et le fuzzing.
//! SysWall daemon library: exposed for integration tests and fuzzing.

#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod bootstrap;
pub mod config;
pub mod grpc;
pub mod signals;
pub mod startup_error;
pub mod supervisor;
pub mod watchdog;
