/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::time::{Duration, Instant};


pub struct RobustThroughputEstimatorSettings {
  // Set `enabled` to true to use the RobustThroughputEstimator, false to use
  // the AcknowledgedBitrateEstimator.
  pub enabled: bool,

  // The estimator keeps the smallest window containing at least
  // `window_packets` and at least the packets received during the last
  // `min_window_duration` milliseconds.
  // (This means that it may store more than `window_packets` at high bitrates,
  // and a longer duration than `min_window_duration` at low bitrates.)
  // However, if will never store more than kMaxPackets (for performance
  // reasons), and never longer than max_window_duration (to avoid very old
  // packets influencing the estimate for example when sending is paused).
  pub window_packets: u64,
  pub max_window_packets: u64,
  pub min_window_duration: Duration,
  pub max_window_duration: Duration,

  // The estimator window requires at least `required_packets` packets
  // to produce an estimate.
  pub required_packets: u64,

  // If audio packets aren't included in allocation (i.e. the
  // estimated available bandwidth is divided only among the video
  // streams), then `unacked_weight` should be set to 0.
  // If audio packets are included in allocation, but not in bandwidth
  // estimation (i.e. they don't have transport-wide sequence numbers,
  // but we nevertheless divide the estimated available bandwidth among
  // both audio and video streams), then `unacked_weight` should be set to 1.
  // If all packets have transport-wide sequence numbers, then the value
  // of `unacked_weight` doesn't matter.
  pub unacked_weight: f64,
}

impl Default for RobustThroughputEstimatorSettings {
  fn default() -> Self {
  Self {
    enabled: true,
    window_packets: 20,
    max_window_packets: 500,
    min_window_duration: Duration::from_secs(1),
    max_window_duration: Duration::from_secs(5),
    required_packets: 10,
    unacked_weight: 1.0,
  }
}
}


pub trait AcknowledgedBitrateEstimatorInterface {
  fn create(key_value_config: &FieldTrialsView) -> Self;
  fn incoming_packet_feedback(&mut self, packet_feedback_vector: &[PacketResult]);
  fn bitrate() -> Option<DataRate>;
  fn peek_rate() -> Option<DataRate>;
  fn set_alr(in_alr: bool);
  fn set_alr_ended_time(alr_ended_time: Instant);
}
