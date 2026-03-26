/// Process resolution module using /proc filesystem.
/// Module de resolution de processus utilisant le systeme de fichiers /proc.
pub mod cache;
pub mod icon_resolver;
pub mod proc_parser;
pub mod resolver;

pub use icon_resolver::IconResolver;
pub use resolver::{ProcfsConfig, ProcfsProcessResolver};
