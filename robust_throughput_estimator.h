/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_ROBUST_THROUGHPUT_ESTIMATOR_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_ROBUST_THROUGHPUT_ESTIMATOR_H_

#include <deque>
#include <optional>
#include <vector>

#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"



impl AcknowledgedBitrateEstimatorInterface for RobustThroughputEstimator {

}

pub struct RobustThroughputEstimator {
 public:
  explicit RobustThroughputEstimator(
      const RobustThroughputEstimatorSettings& settings);
  ~RobustThroughputEstimator() override;

  void IncomingPacketFeedbackVector(
      const Vec<PacketResult>& packet_feedback_vector) override;

  Option<DataRate> bitrate() const override;

  Option<DataRate> PeekRate() const override { return bitrate(); }
  void SetAlr(bool /*in_alr*/) override {}
  void SetAlrEndedTime(Timestamp /*alr_ended_time*/) override {}

 private:
  bool FirstPacketOutsideWindow();

  const RobustThroughputEstimatorSettings self.settings;
  window: VecDeque<PacketResult>,
  Timestamp self.latest_discarded_send_time = Timestamp::MinusInfinity();
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_ROBUST_THROUGHPUT_ESTIMATOR_H_
