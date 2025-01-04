/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::VecDeque;

use crate::{
    api::{
        transport::PacketResult,
        units::{DataRate, TimeDelta, Timestamp},
    },
    AcknowledgedBitrateEstimatorInterface,
};

// WebRTC-Bwe-RobustThroughputEstimatorSettings
#[derive(Debug, Clone)]
pub struct RobustThroughputEstimatorSettings {
    // Set `enabled` to true to use the RobustThroughputEstimator, false to use
    // the AcknowledgedBitrateEstimator.
    pub enabled: bool,

    // The estimator keeps the smallest window containing at least
    // `window_packets` and at least the packets received during the last
    // `min_window_duration` milliseconds.
    // (This means that it may store more than `window_packets` at high bitrates,
    // and a longer duration than `min_window_duration` at low bitrates.)
    // However, if will never store more than MaxPackets (for performance
    // reasons), and never longer than max_window_duration (to avoid very old
    // packets influencing the estimate for example when sending is paused).
    pub window_packets: u64,
    pub max_window_packets: u64,
    pub min_window_duration: TimeDelta,
    pub max_window_duration: TimeDelta,

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
            min_window_duration: TimeDelta::Seconds(1),
            max_window_duration: TimeDelta::Seconds(5),
            required_packets: 10,
            unacked_weight: 1.0,
        }
    }
}

impl RobustThroughputEstimatorSettings {
    pub fn validate(&mut self) {
        if (self.window_packets < 10 || 1000 < self.window_packets) {
            tracing::warn!("Window size must be between 10 and 1000 packets");
            self.window_packets = 20;
          }
          if (self.max_window_packets < 10 || 1000 < self.max_window_packets) {
            tracing::warn!("Max window size must be between 10 and 1000 packets");
            self.max_window_packets = 500;
          }
          self.max_window_packets = self.max_window_packets.max(self.window_packets);

          if self.required_packets < 10 || 1000 < self.required_packets {
            tracing::warn!("Required number of initial packets must be between 10 and 1000 packets");
            self.required_packets = 10;
          }
          self.required_packets = self.required_packets.min(self.window_packets);

          if (self.min_window_duration < TimeDelta::Millis(100) ||
              TimeDelta::Millis(3000) < self.min_window_duration) {
            tracing::warn!("Window duration must be between 100 and 3000 ms");
            self.min_window_duration = TimeDelta::Millis(750);
          }
          if (self.max_window_duration < TimeDelta::Seconds(1) ||
              TimeDelta::Seconds(15) < self.max_window_duration) {
            tracing::warn!("Max window duration must be between 1 and 15 s");
            self.max_window_duration = TimeDelta::Seconds(5);
          }
          self.min_window_duration = self.min_window_duration.min(self.max_window_duration);

          if (self.unacked_weight < 0.0 || 1.0 < self.unacked_weight) {
            tracing::warn!("Weight for prior unacked size must be between 0 and 1.");
            self.unacked_weight = 1.0;
          }
    }
}

pub struct RobustThroughputEstimator {
    settings: RobustThroughputEstimatorSettings,
    window: VecDeque<PacketResult>,
    latest_discarded_send_time: Timestamp,
}

impl Default for RobustThroughputEstimator {
    fn default() -> Self {
        Self::new(RobustThroughputEstimatorSettings::default())
    }
}

impl RobustThroughputEstimator {
    pub fn new(mut settings: RobustThroughputEstimatorSettings) -> Self {
        settings.validate();

        Self {
            settings,
            window: VecDeque::new(),
            latest_discarded_send_time: Timestamp::MinusInfinity(),
        }
    }

    fn FirstPacketOutsideWindow() -> bool {
        todo!();
    }
}

impl AcknowledgedBitrateEstimatorInterface for RobustThroughputEstimator {
    fn incoming_packet_feedback(&mut self, packet_feedback_vector: &[PacketResult]) {
        todo!();
    }

    fn bitrate(&self) -> Option<DataRate> {
        todo!()
    }

    fn peek_rate(&self) -> Option<DataRate> {
        todo!()
    }

    fn set_alr(&mut self, in_alr: bool) {
        todo!()
    }

    fn set_alr_ended_time(&mut self, alr_ended_time: crate::api::units::Timestamp) {
        todo!()
    }
}
