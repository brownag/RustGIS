//! The R harness: the bootstrap script that runs each job, embedded as `BOOTSTRAP_R`.

pub const BOOTSTRAP_R: &str = include_str!("bootstrap.R");
