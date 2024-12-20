/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_PROBE_BITRATE_ESTIMATOR_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_PROBE_BITRATE_ESTIMATOR_H_

#include <map>
#include <optional>

#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/timestamp.h"


class RtcEventLog;

pub struct ProbeBitrateEstimator {
 public:
  explicit ProbeBitrateEstimator(RtcEventLog* event_log);
  ~ProbeBitrateEstimator();

  // Should be called for every probe packet we receive feedback about.
  // Returns the estimated bitrate if the probe completes a valid cluster.
  Option<DataRate> HandleProbeAndEstimateBitrate(
      const PacketResult& packet_feedback);

  Option<DataRate> FetchAndResetLastEstimatedBitrate();

 private:
  struct AggregatedCluster {
    let num_probes: isize = 0;
    let first_send: Timestamp = Timestamp::PlusInfinity();
    let last_send: Timestamp = Timestamp::MinusInfinity();
    let first_receive: Timestamp = Timestamp::PlusInfinity();
    let last_receive: Timestamp = Timestamp::MinusInfinity();
    let size_last_send: DataSize = DataSize::Zero();
    let size_first_receive: DataSize = DataSize::Zero();
    let usizeotal: DataSize = DataSize::Zero();
  };

  // Erases old cluster data that was seen before `timestamp`.
  fn EraseOldClusters(Timestamp timestamp) {
  todo!();
}

  std::map<int, AggregatedCluster> self.clusters;
  event_log: RtcEventLog*,
  estimated_data_rate: Option<DataRate>,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_PROBE_BITRATE_ESTIMATOR_H_
