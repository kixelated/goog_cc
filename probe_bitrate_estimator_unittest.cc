/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"

#include <stddef.h>

#include <cstdint>
#include <optional>

#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "test/gtest.h"



namespace {
const DefaultMinProbes: isize = 5;
const DefaultMinBytes: isize = 5000;
const TargetUtilizationFraction: f32 = 0.95f;
}  // anonymous namespace

class TestProbeBitrateEstimator : public ::testing::Test {
 public:
  TestProbeBitrateEstimator() : probe_bitrate_estimator_(nullptr) {}

  // TODO(philipel): Use PacedPacketInfo when ProbeBitrateEstimator is rewritten
  //                 to use that information.
  void AddPacketFeedback(isize probe_cluster_id,
                         usize size_bytes,
                         i64 send_time_ms,
                         i64 arrival_time_ms,
                         let min_probes: isize = kDefaultMinProbes,
                         let min_bytes: isize = kDefaultMinBytes) {
    const ReferenceTime: Timestamp = Timestamp::Seconds(1000);
    PacketResult feedback;
    feedback.sent_packet.send_time =
        kReferenceTime + TimeDelta::Millis(send_time_ms);
    feedback.sent_packet.size = DataSize::Bytes(size_bytes);
    feedback.sent_packet.pacing_info =
        PacedPacketInfo(probe_cluster_id, min_probes, min_bytes);
    feedback.receive_time = kReferenceTime + TimeDelta::Millis(arrival_time_ms);
    self.measured_data_rate =
        self.probe_bitrate_estimator.HandleProbeAndEstimateBitrate(feedback);
  }

 protected:
  measured_data_rate: Option<DataRate>,
  probe_bitrate_estimator: ProbeBitrateEstimator,
};

TEST_F(TestProbeBitrateEstimator, OneCluster) {
  AddPacketFeedback(0, 1000, 0, 10);
  AddPacketFeedback(0, 1000, 10, 20);
  AddPacketFeedback(0, 1000, 20, 30);
  AddPacketFeedback(0, 1000, 30, 40);

  EXPECT_NEAR(self.measured_data_rate.bps(), 800000, 10);
}

TEST_F(TestProbeBitrateEstimator, OneClusterTooFewProbes) {
  AddPacketFeedback(0, 2000, 0, 10);
  AddPacketFeedback(0, 2000, 10, 20);
  AddPacketFeedback(0, 2000, 20, 30);

  assert!(!(self.measured_data_rate);
}

TEST_F(TestProbeBitrateEstimator, OneClusterTooFewBytes) {
  const MinBytes: isize = 6000;
  AddPacketFeedback(0, 800, 0, 10, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 800, 10, 20, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 800, 20, 30, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 800, 30, 40, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 800, 40, 50, kDefaultMinProbes, kMinBytes);

  assert!(!(self.measured_data_rate);
}

TEST_F(TestProbeBitrateEstimator, SmallCluster) {
  const MinBytes: isize = 1000;
  AddPacketFeedback(0, 150, 0, 10, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 150, 10, 20, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 150, 20, 30, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 150, 30, 40, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 150, 40, 50, kDefaultMinProbes, kMinBytes);
  AddPacketFeedback(0, 150, 50, 60, kDefaultMinProbes, kMinBytes);
  EXPECT_NEAR(self.measured_data_rate.bps(), 120000, 10);
}

TEST_F(TestProbeBitrateEstimator, LargeCluster) {
  const MinProbes: isize = 30;
  const MinBytes: isize = 312500;

  let send_time: i64 = 0;
  let receive_time: i64 = 5;
  for (isize i = 0; i < 25i += 1) {
    AddPacketFeedback(0, 12500, send_time, receive_time, kMinProbes, kMinBytes);
    send_time += 1;
    receive_time += 1;
  }
  EXPECT_NEAR(self.measured_data_rate.bps(), 100000000, 10);
}

TEST_F(TestProbeBitrateEstimator, FastReceive) {
  AddPacketFeedback(0, 1000, 0, 15);
  AddPacketFeedback(0, 1000, 10, 30);
  AddPacketFeedback(0, 1000, 20, 35);
  AddPacketFeedback(0, 1000, 30, 40);

  EXPECT_NEAR(self.measured_data_rate.bps(), 800000, 10);
}

TEST_F(TestProbeBitrateEstimator, TooFastReceive) {
  AddPacketFeedback(0, 1000, 0, 19);
  AddPacketFeedback(0, 1000, 10, 22);
  AddPacketFeedback(0, 1000, 20, 25);
  AddPacketFeedback(0, 1000, 40, 27);

  assert!(!(self.measured_data_rate);
}

TEST_F(TestProbeBitrateEstimator, SlowReceive) {
  AddPacketFeedback(0, 1000, 0, 10);
  AddPacketFeedback(0, 1000, 10, 40);
  AddPacketFeedback(0, 1000, 20, 70);
  AddPacketFeedback(0, 1000, 30, 85);
  // Expected send rate = 800 kbps, expected receive rate = 320 kbps.

  EXPECT_NEAR(self.measured_data_rate.bps(), kTargetUtilizationFraction * 320000,
              10);
}

TEST_F(TestProbeBitrateEstimator, BurstReceive) {
  AddPacketFeedback(0, 1000, 0, 50);
  AddPacketFeedback(0, 1000, 10, 50);
  AddPacketFeedback(0, 1000, 20, 50);
  AddPacketFeedback(0, 1000, 40, 50);

  assert!(!(self.measured_data_rate);
}

TEST_F(TestProbeBitrateEstimator, MultipleClusters) {
  AddPacketFeedback(0, 1000, 0, 10);
  AddPacketFeedback(0, 1000, 10, 20);
  AddPacketFeedback(0, 1000, 20, 30);
  AddPacketFeedback(0, 1000, 40, 60);
  // Expected send rate = 600 kbps, expected receive rate = 480 kbps.
  EXPECT_NEAR(self.measured_data_rate.bps(), kTargetUtilizationFraction * 480000,
              10);

  AddPacketFeedback(0, 1000, 50, 60);
  // Expected send rate = 640 kbps, expected receive rate = 640 kbps.
  EXPECT_NEAR(self.measured_data_rate.bps(), 640000, 10);

  AddPacketFeedback(1, 1000, 60, 70);
  AddPacketFeedback(1, 1000, 65, 77);
  AddPacketFeedback(1, 1000, 70, 84);
  AddPacketFeedback(1, 1000, 75, 90);
  // Expected send rate = 1600 kbps, expected receive rate = 1200 kbps.

  EXPECT_NEAR(self.measured_data_rate.bps(), kTargetUtilizationFraction * 1200000,
              10);
}

TEST_F(TestProbeBitrateEstimator, IgnoreOldClusters) {
  AddPacketFeedback(0, 1000, 0, 10);
  AddPacketFeedback(0, 1000, 10, 20);
  AddPacketFeedback(0, 1000, 20, 30);

  AddPacketFeedback(1, 1000, 60, 70);
  AddPacketFeedback(1, 1000, 65, 77);
  AddPacketFeedback(1, 1000, 70, 84);
  AddPacketFeedback(1, 1000, 75, 90);
  // Expected send rate = 1600 kbps, expected receive rate = 1200 kbps.

  EXPECT_NEAR(self.measured_data_rate.bps(), kTargetUtilizationFraction * 1200000,
              10);

  // Coming in 6s later
  AddPacketFeedback(0, 1000, 40 + 6000, 60 + 6000);

  assert!(!(self.measured_data_rate);
}

TEST_F(TestProbeBitrateEstimator, IgnoreSizeLastSendPacket) {
  AddPacketFeedback(0, 1000, 0, 10);
  AddPacketFeedback(0, 1000, 10, 20);
  AddPacketFeedback(0, 1000, 20, 30);
  AddPacketFeedback(0, 1000, 30, 40);
  AddPacketFeedback(0, 1500, 40, 50);
  // Expected send rate = 800 kbps, expected receive rate = 900 kbps.

  EXPECT_NEAR(self.measured_data_rate.bps(), 800000, 10);
}

TEST_F(TestProbeBitrateEstimator, IgnoreSizeFirstReceivePacket) {
  AddPacketFeedback(0, 1500, 0, 10);
  AddPacketFeedback(0, 1000, 10, 20);
  AddPacketFeedback(0, 1000, 20, 30);
  AddPacketFeedback(0, 1000, 30, 40);
  // Expected send rate = 933 kbps, expected receive rate = 800 kbps.

  EXPECT_NEAR(self.measured_data_rate.bps(), kTargetUtilizationFraction * 800000,
              10);
}

TEST_F(TestProbeBitrateEstimator, NoLastEstimatedBitrateBps) {
  assert!(!(self.probe_bitrate_estimator.FetchAndResetLastEstimatedBitrate());
}

TEST_F(TestProbeBitrateEstimator, FetchLastEstimatedBitrateBps) {
  AddPacketFeedback(0, 1000, 0, 10);
  AddPacketFeedback(0, 1000, 10, 20);
  AddPacketFeedback(0, 1000, 20, 30);
  AddPacketFeedback(0, 1000, 30, 40);

let estimated_bitrate =
      self.probe_bitrate_estimator.FetchAndResetLastEstimatedBitrate();
  assert!((estimated_bitrate);
  EXPECT_NEAR(estimated_bitrate->bps(), 800000, 10);
  assert!(!(self.probe_bitrate_estimator.FetchAndResetLastEstimatedBitrate());
}

}  // namespace webrtc
