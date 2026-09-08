//! RelAudio çekirdeği — ses yakalama, çalma ve ağ taşıma.
//!
//! Mimari: `docs/02-mimari.md`. Bu crate arayüzden bağımsızdır ve ayrı bir
//! süreçte çalışacak şekilde tasarlanmıştır (docs/adr/0002).

pub mod audio;
pub mod config;
pub mod error;
pub mod net;

pub use config::Config;
pub use error::{Error, Result};
