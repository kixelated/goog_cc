//! # Google Congestion Control
//!
//! This crate is a Rust port of the [Google Congestion Control](https://webrtc.googlesource.com/src/+/refs/heads/main/modules/congestion_controller/goog_cc/) (GCC) implementation from the WebRTC project.
//! No more painful linking against libwebrtc.
//!
//! Google Congestion Control is the congestion control algorithm used in WebRTC.
//! It is a hybrid algorithm that combines the strengths of delay-based and loss-based congestion control.
//! The end goal is minimum latency and to avoid buffer-bloat as much as possible.
//!
//! All licensing from the original source code is preserved under a BSD-style license.
//! Thanks to the WebRTC project authors for their hard work on this implementation.
//! All I did was port it to Rust.

mod api;

/// [FieldTrials](experiments::FieldTrials) and dynamic configuration.
pub mod experiments;
mod goog_cc;
mod pacing;
mod remote_bitrate_estimator;
mod rtc;

// Selectively export the public API.
pub use api::{network_control, transport, units};
pub use goog_cc::{GoogCcConfig, GoogCcNetworkController};
