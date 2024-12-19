/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/trendline_estimator.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <vector>

#include "api/transport/bandwidth_usage.h"
#include "api/transport/field_trial_based_config.h"
#include "rtc_base/checks.h"
#include "test/gtest.h"


namespace {

pub struct PacketTimeGenerator {
 public:
  PacketTimeGenerator(i64 initial_clock, f64 time_between_packets)
      : initial_clock_(initial_clock),
        time_between_packets_(time_between_packets),
        packets_(0) {}
  i64 operator()() {
    return self.initial_clock + self.time_between_packets * self.packets += 1;
  }

 private:
  const i64 self.initial_clock;
  f64 self.time_between_packets;
  packets: usize,
};

class TrendlineEstimatorTest : public testing::Test {
 public:
  TrendlineEstimatorTest()
      : send_times(kPacketCount),
        recv_times(kPacketCount),
        packet_sizes(kPacketCount),
        config(),
        estimator(&config, nullptr),
        count(1) {
    std::fill(packet_sizes.begin(), packet_sizes.end(), kPacketSizeBytes);
  }

fn RunTestUntilStateChange() -> void {
    assert!_EQ(send_times.len(), kPacketCount);
    assert!_EQ(recv_times.len(), kPacketCount);
    assert!_EQ(packet_sizes.len(), kPacketCount);
    assert!_GE(count, 1);
    assert!_LT(count, kPacketCount);

let initial_state = estimator.State();
    for (; count < kPacketCount; count++) {
      let recv_delta: f64 = recv_times[count] - recv_times[count - 1];
      let send_delta: f64 = send_times[count] - send_times[count - 1];
      estimator.Update(recv_delta, send_delta, send_times[count],
                       recv_times[count], packet_sizes[count], true);
      if (estimator.State() != initial_state) {
        return;
      }
    }
  }

 protected:
  const usize kPacketCount = 25;
  const usize kPacketSizeBytes = 1200;
  Vec<i64> send_times;
  Vec<i64> recv_times;
  Vec<usize> packet_sizes;
  const FieldTrialBasedConfig config;
  TrendlineEstimator estimator;
  usize count;
};
}  // namespace

TEST_F(TrendlineEstimatorTest, Normal) {
  PacketTimeGenerator send_time_generator(123456789 /*initial clock*/,
                                          20 /*20 ms between sent packets*/);
  std::generate(send_times.begin(), send_times.end(), send_time_generator);

  PacketTimeGenerator recv_time_generator(987654321 /*initial clock*/,
                                          20 /*delivered at the same pace*/);
  std::generate(recv_times.begin(), recv_times.end(), recv_time_generator);

  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwNormal);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwNormal);
  EXPECT_EQ(count, kPacketCount);  // All packets processed
}

TEST_F(TrendlineEstimatorTest, Overusing) {
  PacketTimeGenerator send_time_generator(123456789 /*initial clock*/,
                                          20 /*20 ms between sent packets*/);
  std::generate(send_times.begin(), send_times.end(), send_time_generator);

  PacketTimeGenerator recv_time_generator(987654321 /*initial clock*/,
                                          1.1 * 20 /*10% slower delivery*/);
  std::generate(recv_times.begin(), recv_times.end(), recv_time_generator);

  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwNormal);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwOverusing);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwOverusing);
  EXPECT_EQ(count, kPacketCount);  // All packets processed
}

TEST_F(TrendlineEstimatorTest, Underusing) {
  PacketTimeGenerator send_time_generator(123456789 /*initial clock*/,
                                          20 /*20 ms between sent packets*/);
  std::generate(send_times.begin(), send_times.end(), send_time_generator);

  PacketTimeGenerator recv_time_generator(987654321 /*initial clock*/,
                                          0.85 * 20 /*15% faster delivery*/);
  std::generate(recv_times.begin(), recv_times.end(), recv_time_generator);

  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwNormal);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwUnderusing);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwUnderusing);
  EXPECT_EQ(count, kPacketCount);  // All packets processed
}

TEST_F(TrendlineEstimatorTest, IncludesSmallPacketsByDefault) {
  PacketTimeGenerator send_time_generator(123456789 /*initial clock*/,
                                          20 /*20 ms between sent packets*/);
  std::generate(send_times.begin(), send_times.end(), send_time_generator);

  PacketTimeGenerator recv_time_generator(987654321 /*initial clock*/,
                                          1.1 * 20 /*10% slower delivery*/);
  std::generate(recv_times.begin(), recv_times.end(), recv_time_generator);

  std::fill(packet_sizes.begin(), packet_sizes.end(), 100);

  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwNormal);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwOverusing);
  RunTestUntilStateChange();
  EXPECT_EQ(estimator.State(), BandwidthUsage::kBwOverusing);
  EXPECT_EQ(count, kPacketCount);  // All packets processed
}

}  // namespace webrtc
