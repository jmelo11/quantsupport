//! Bounded background execution. Native market objects never cross threads.
pub mod scheduler;

#[cfg(windows)]
pub mod bridge;
