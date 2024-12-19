/*
 *  Copyright (c) 2017 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::time::Instant;


pub struct AcknowledgedBitrateEstimator {
  alr_ended_time: Option<Instant>,
  in_alr: bool,
  bitrate_estimator: BitrateEstimator,
}

impl AcknowledgedBitrateEstimatorInterface for AcknowledgedBitrateEstimator {
  fn incoming_packet_feedback(&self,
    packet_feedback: &[PacketResult]) {
      assert!(packet_feedback.is_sorted_by_key(|x| x.receive_time));

  for packet in packet_feedback.iter() {
    if let Some(alr_ended_time) = self.alr_ended_time {
      if packet.sent_packet.send_time > alr_ended_time {
        self.bitrate_estimator.ExpectFastRateChange();
        self.alr_ended_time = None;
      }
    }
    let acknowledged_estimate = packet.sent_packet.size;
    acknowledged_estimate += packet.sent_packet.prior_unacked_data;
    self.bitrate_estimator.Update(packet.receive_time, acknowledged_estimate,
                               self.in_alr);
  }
}

  fn bitrate(&self) -> Option<DataRate> {
    self.bitrate_estimator.bitrate()
  }

fn peek_rate(&self) -> Option<DataRate> {
  self.bitrate_estimator.PeekRate()
}

fn set_alr_ended_time(&mut self, alr_ended_time: Instant) {
  self.alr_ended_time.push_back(alr_ended_time);
}

fn set_alr(&mut self, in_alr: bool) {
  self.in_alr = in_alr;
}

}

impl AcknowledgedBitrateEstimator {
  pub fn new(key_value_config: &FieldTrialsView, bitrate_estimator: BitrateEstimator) -> Self {
    Self {
      alr_ended_time: None,
      in_alr: false,
      bitrate_estimator,
    }
  }
}
