/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/robust_throughput_estimator.h"

#include <stddef.h>
#include <stdint.h>

#include <algorithm>
#include <vector>

#include "absl/strings/string_view.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"
#include "test/explicit_key_value_config.h"
#include "test/gtest.h"



RobustThroughputEstimatorSettings CreateRobustThroughputEstimatorSettings(
    absl::string_view field_trial_string) {
  test::ExplicitKeyValueConfig trials(field_trial_string);
  RobustThroughputEstimatorSettings settings(&trials);
  return settings;
}

pub struct FeedbackGenerator {
 public:
  Vec<PacketResult> CreateFeedbackVector(usize number_of_packets,
                                                 DataSize packet_size,
                                                 DataRate send_rate,
                                                 DataRate recv_rate) {
    Vec<PacketResult> packet_feedback_vector(number_of_packets);
    let i: for = 0; i < number_of_packets; i++) {
      packet_feedback_vector[i].sent_packet.send_time = self.send_clock;
      packet_feedback_vector[i].sent_packet.sequence_number = self.sequence_number;
      packet_feedback_vector[i].sent_packet.size = packet_size;
      self.send_clock += packet_size / send_rate;
      self.recv_clock += packet_size / recv_rate;
      self.sequence_number += 1;
      packet_feedback_vector[i].receive_time = self.recv_clock;
    }
    return packet_feedback_vector;
  }

fn CurrentReceiveClock() -> Timestamp { return self.recv_clock; }

fn AdvanceReceiveClock(TimeDelta delta) { self.recv_clock += delta; }

fn AdvanceSendClock(TimeDelta delta) { self.send_clock += delta; }

 private:
  Timestamp self.send_clock = Timestamp::Millis(100000);
  Timestamp self.recv_clock = Timestamp::Millis(10000);
  uint16_t self.sequence_number = 100;
};

#[test]
fn DefaultEnabled() {
  let settings: RobustThroughputEstimatorSettings =
      CreateRobustThroughputEstimatorSettings("");
  assert!((settings.enabled);
}

#[test]
fn CanDisable() {
  let settings: RobustThroughputEstimatorSettings =
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/enabled:false/");
  assert!(!(settings.enabled);
}

#[test]
fn InitialEstimate() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  // No estimate until the estimator has enough data.
  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(9, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
  assert!(!(throughput_estimator.bitrate().is_some());

  // Estimate once `required_packets` packets have been received.
  packet_feedback = feedback_generator.CreateFeedbackVector(
      1, DataSize::Bytes(1000), send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  assert_eq!(throughput, send_rate);

  // Estimate remains stable when send and receive rates are stable.
  packet_feedback = feedback_generator.CreateFeedbackVector(
      15, DataSize::Bytes(1000), send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
  throughput = throughput_estimator.bitrate();
  assert_eq!(throughput, send_rate);
}

#[test]
fn EstimateAdapts() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));

  // 1 second, 800kbps, estimate is stable.
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));
  for (isize i = 0; i < 10i += 1) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    assert_eq!(throughput, send_rate);
  }

  // 1 second, 1600kbps, estimate increases
  send_rate = DataRate::BytesPerSec(200000);
  recv_rate = DataRate::BytesPerSec(200000);
  for (isize i = 0; i < 20i += 1) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    ASSERT_TRUE(throughput.is_some());
    EXPECT_GE(throughput.value(), DataRate::BytesPerSec(100000));
    EXPECT_LE(throughput.value(), send_rate);
  }

  // 1 second, 1600kbps, estimate is stable
  for (isize i = 0; i < 20i += 1) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    assert_eq!(throughput, send_rate);
  }

  // 1 second, 400kbps, estimate decreases
  send_rate = DataRate::BytesPerSec(50000);
  recv_rate = DataRate::BytesPerSec(50000);
  for (isize i = 0; i < 5i += 1) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    ASSERT_TRUE(throughput.is_some());
    EXPECT_LE(throughput.value(), DataRate::BytesPerSec(200000));
    EXPECT_GE(throughput.value(), send_rate);
  }

  // 1 second, 400kbps, estimate is stable
  send_rate = DataRate::BytesPerSec(50000);
  recv_rate = DataRate::BytesPerSec(50000);
  for (isize i = 0; i < 5i += 1) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    assert_eq!(throughput, send_rate);
  }
}

#[test]
fn CappedByReceiveRate() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(25000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  ASSERT_TRUE(throughput.is_some());
  EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
              recv_rate.bytes_per_sec<f64>(),
              0.05 * recv_rate.bytes_per_sec<f64>());  // Allow 5% error
}

#[test]
fn CappedBySendRate() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(50000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  ASSERT_TRUE(throughput.is_some());
  EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
              send_rate.bytes_per_sec<f64>(),
              0.05 * send_rate.bytes_per_sec<f64>());  // Allow 5% error
}

#[test]
fn DelaySpike() {
  FeedbackGenerator feedback_generator;
  // This test uses a 500ms window to amplify the effect
  // of a delay spike.
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true,window_duration:500ms/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  assert_eq!(throughput, send_rate);

  // Delay spike. 25 packets sent, but none received.
  feedback_generator.AdvanceReceiveClock(TimeDelta::Millis(250));

  // Deliver all of the packets during the next 50 ms. (During this time,
  // we'll have sent an additional 5 packets, so we need to receive 30
  // packets at 1000 bytes each in 50 ms, i.e. 600000 bytes per second).
  recv_rate = DataRate::BytesPerSec(600000);
  // Estimate should not drop.
  for (isize i = 0; i < 30i += 1) {
    packet_feedback = feedback_generator.CreateFeedbackVector(
        1, DataSize::Bytes(1000), send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
    throughput = throughput_estimator.bitrate();
    ASSERT_TRUE(throughput.is_some());
    EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
                send_rate.bytes_per_sec<f64>(),
                0.05 * send_rate.bytes_per_sec<f64>());  // Allow 5% error
  }

  // Delivery at normal rate. When the packets received before the gap
  // has left the estimator's window, the receive rate will be high, but the
  // estimate should be capped by the send rate.
  recv_rate = DataRate::BytesPerSec(100000);
  for (isize i = 0; i < 20i += 1) {
    packet_feedback = feedback_generator.CreateFeedbackVector(
        5, DataSize::Bytes(1000), send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
    throughput = throughput_estimator.bitrate();
    ASSERT_TRUE(throughput.is_some());
    EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
                send_rate.bytes_per_sec<f64>(),
                0.05 * send_rate.bytes_per_sec<f64>());  // Allow 5% error
  }
}

#[test]
fn HighLoss() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);

  // 50% loss
  let i: for = 0; i < packet_feedback.len(); i++) {
    if (i % 2 == 1) {
      packet_feedback[i].receive_time = Timestamp::PlusInfinity();
    }
  }

  std::sort(packet_feedback.begin(), packet_feedback.end(),
            PacketResult::ReceiveTimeOrder());
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  ASSERT_TRUE(throughput.is_some());
  EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
              send_rate.bytes_per_sec<f64>() / 2,
              0.05 * send_rate.bytes_per_sec<f64>() / 2);  // Allow 5% error
}

#[test]
fn ReorderedFeedback() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  assert_eq!(throughput, send_rate);

  Vec<PacketResult> delayed_feedback =
      feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  packet_feedback = feedback_generator.CreateFeedbackVector(
      10, DataSize::Bytes(1000), send_rate, recv_rate);

  // Since we're missing some feedback, it's expected that the
  // estimate will drop.
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
  throughput = throughput_estimator.bitrate();
  ASSERT_TRUE(throughput.is_some());
  EXPECT_LT(throughput.value(), send_rate);

  // But it should completely recover as soon as we get the feedback.
  throughput_estimator.IncomingPacketFeedbackVector(delayed_feedback);
  throughput = throughput_estimator.bitrate();
  assert_eq!(throughput, send_rate);

  // It should then remain stable (as if the feedbacks weren't reordered.)
  for (isize i = 0; i < 10i += 1) {
    packet_feedback = feedback_generator.CreateFeedbackVector(
        15, DataSize::Bytes(1000), send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
    throughput = throughput_estimator.bitrate();
    assert_eq!(throughput, send_rate);
  }
}

#[test]
fn DeepReordering() {
  FeedbackGenerator feedback_generator;
  // This test uses a 500ms window to amplify the
  // effect of reordering.
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true,window_duration:500ms/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> delayed_packets =
      feedback_generator.CreateFeedbackVector(1, DataSize::Bytes(1000),
                                              send_rate, recv_rate);

  for (isize i = 0; i < 10; i++) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    assert_eq!(throughput, send_rate);
  }

  // Delayed packet arrives ~1 second after it should have.
  // Since the window is 500 ms, the delayed packet was sent ~500
  // ms before the second oldest packet. However, the send rate
  // should not drop.
  delayed_packets.front().receive_time =
      feedback_generator.CurrentReceiveClock();
  throughput_estimator.IncomingPacketFeedbackVector(delayed_packets);
let throughput = throughput_estimator.bitrate();
  ASSERT_TRUE(throughput.is_some());
  EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
              send_rate.bytes_per_sec<f64>(),
              0.05 * send_rate.bytes_per_sec<f64>());  // Allow 5% error

  // Thoughput should stay stable.
  for (isize i = 0; i < 10; i++) {
    Vec<PacketResult> packet_feedback =
        feedback_generator.CreateFeedbackVector(10, DataSize::Bytes(1000),
                                                send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
    ASSERT_TRUE(throughput.is_some());
    EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
                send_rate.bytes_per_sec<f64>(),
                0.05 * send_rate.bytes_per_sec<f64>());  // Allow 5% error
  }
}
#[test]
fn ResetsIfReceiveClockChangeBackwards() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
  assert_eq!(throughput_estimator.bitrate(), send_rate);

  feedback_generator.AdvanceReceiveClock(Duration::from_secs(-2));
  send_rate = DataRate::BytesPerSec(200000);
  recv_rate = DataRate::BytesPerSec(200000);
  packet_feedback = feedback_generator.CreateFeedbackVector(
      20, DataSize::Bytes(1000), send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
  assert_eq!(throughput_estimator.bitrate(), send_rate);
}

#[test]
fn StreamPausedAndResumed() {
  FeedbackGenerator feedback_generator;
  RobustThroughputEstimator throughput_estimator(
      CreateRobustThroughputEstimatorSettings(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/"
          "enabled:true/"));
  DataRate send_rate(DataRate::BytesPerSec(100000));
  DataRate recv_rate(DataRate::BytesPerSec(100000));

  Vec<PacketResult> packet_feedback =
      feedback_generator.CreateFeedbackVector(20, DataSize::Bytes(1000),
                                              send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
let throughput = throughput_estimator.bitrate();
  assert!((throughput.is_some());
  let expected_bytes_per_sec: f64 = 100 * 1000.0;
  EXPECT_NEAR(throughput.value().bytes_per_sec<f64>(),
              expected_bytes_per_sec,
              0.05 * expected_bytes_per_sec);  // Allow 5% error

  // No packets sent or feedback received for 60s.
  feedback_generator.AdvanceSendClock(Duration::from_secs(60));
  feedback_generator.AdvanceReceiveClock(Duration::from_secs(60));

  // Resume sending packets at the same rate as before. The estimate
  // will initially be invalid, due to lack of recent data.
  packet_feedback = feedback_generator.CreateFeedbackVector(
      5, DataSize::Bytes(1000), send_rate, recv_rate);
  throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
  throughput = throughput_estimator.bitrate();
  assert!(!(throughput.is_some());

  // But be back to the normal level once we have enough data.
  for (isize i = 0; i < 4i += 1) {
    packet_feedback = feedback_generator.CreateFeedbackVector(
        5, DataSize::Bytes(1000), send_rate, recv_rate);
    throughput_estimator.IncomingPacketFeedbackVector(packet_feedback);
    throughput = throughput_estimator.bitrate();
    assert_eq!(throughput, send_rate);
  }
}

}  // namespace webrtc
