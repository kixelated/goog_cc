[![crates.io](https://img.shields.io/crates/v/goog_cc)](https://crates.io/crates/goog_cc)
[![docs.rs](https://img.shields.io/docsrs/goog_cc)](https://docs.rs/goog_cc)

# Google Congestion Control

This crate is a Rust port of the [Google Congestion Control](https://webrtc.googlesource.com/src/+/refs/heads/main/modules/congestion_controller/goog_cc/) (GCC) implementation from the WebRTC project.
No more painful linking against libwebrtc.

Google Congestion Control is the congestion control algorithm used in WebRTC.
It is a hybrid algorithm that combines the strengths of delay-based and loss-based congestion control.
The end goal is minimum latency and to avoid buffer-bloat as much as possible.

All licensing from the original source code is preserved under a BSD-style license.
Thanks to the WebRTC project authors for their hard work on this implementation.
All I did was port it to Rust.

## Updates
I will periodically update this crate with the latest changes from the WebRTC project.

The current version: [da2a3d6c4d](https://webrtc.googlesource.com/src/+/da2a3d6c4d) (2024-12-09)

## Notable Changes
**NetworkStateEstimator and NetworkStatePredictor have been removed.**
They are not implemented in the WebRTC source code and are flagged with a "subject to change" comment.
I think this is a back-door for Google to tweak the congestion control algorithm?

**FieldTrials is a type-safe struct instead of a string.**
The original implementation uses a string to represent field trials.
This was quite error-prone as many trials do not match the cooresponding field name.
I decided to create a [FieldTrials](experiments::FieldTrials) struct to represent field trials in a type-safe way.

**RtcEventLog has been removed.**
The structured logs are cool, but have been removed for now to simplify the code.

**RTC_LOG has been replaced with [tracing].**
Use [tracing_subscriber](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/index.html) to enable and configure the logger.
