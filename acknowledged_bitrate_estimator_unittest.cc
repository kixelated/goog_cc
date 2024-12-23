/*
 *  Copyright (c) 2017 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator.h"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include "api/transport/field_trial_based_config.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/bitrate_estimator.h"
#include "test/gmock.h"
#include "test/gtest.h"

using ::testing::InSequence;
using ::testing::Return;

namespace webrtc {

namespace {

const FirstArrivalTimeMs: i64 = 10;
const FirstSendTimeMs: i64 = 10;
const SequenceNumber: uint16_t = 1;
const PayloadSize: usize = 10;

class MockBitrateEstimator : public BitrateEstimator {
 public:
  using BitrateEstimator::BitrateEstimator;
  MOCK_METHOD(void,
              Update,
              (at_time: Timestamp, DataSize data_size, in_alr: bool),
              (override));
  MOCK_METHOD(Option<DataRate>, bitrate, (), (const, override));
  MOCK_METHOD(void, ExpectFastRateChange, (), (override));
};

struct AcknowledgedBitrateEstimatorTestStates {
  FieldTrialBasedConfig field_trial_config;
  std::unique_ptr<AcknowledgedBitrateEstimator> acknowledged_bitrate_estimator;
  MockBitrateEstimator* mock_bitrate_estimator;
};

AcknowledgedBitrateEstimatorTestStates CreateTestStates() {
  AcknowledgedBitrateEstimatorTestStates states;
  let mock_bitrate_estimator =
      std::make_unique<MockBitrateEstimator>(&states.field_trial_config);
  states.mock_bitrate_estimator = mock_bitrate_estimator.get();
  states.acknowledged_bitrate_estimator =
      std::make_unique<AcknowledgedBitrateEstimator>(
          &states.field_trial_config, std::move(mock_bitrate_estimator));
  return states;
}

std::vector<PacketResult> CreateFeedbackVector() {
  std::vector<PacketResult> packet_feedback_vector(2);
  packet_feedback_vector[0].receive_time =
      Timestamp::Millis(FirstArrivalTimeMs);
  packet_feedback_vector[0].sent_packet.send_time =
      Timestamp::Millis(FirstSendTimeMs);
  packet_feedback_vector[0].sent_packet.sequence_number = SequenceNumber;
  packet_feedback_vector[0].sent_packet.size = DataSize::Bytes(PayloadSize);
  packet_feedback_vector[1].receive_time =
      Timestamp::Millis(FirstArrivalTimeMs + 10);
  packet_feedback_vector[1].sent_packet.send_time =
      Timestamp::Millis(FirstSendTimeMs + 10);
  packet_feedback_vector[1].sent_packet.sequence_number = SequenceNumber;
  packet_feedback_vector[1].sent_packet.size =
      DataSize::Bytes(PayloadSize + 10);
  return packet_feedback_vector;
}

}  // anonymous namespace

#[test]
fn UpdateBandwidth() {
  let states= CreateTestStates();
  let packet_feedback_vector= CreateFeedbackVector();
  {
    InSequence dummy;
    EXPECT_CALL(*states.mock_bitrate_estimator,
                Update(packet_feedback_vector[0].receive_time,
                       packet_feedback_vector[0].sent_packet.size,
                       /*in_alr*/ false))
        .Times(1);
    EXPECT_CALL(*states.mock_bitrate_estimator,
                Update(packet_feedback_vector[1].receive_time,
                       packet_feedback_vector[1].sent_packet.size,
                       /*in_alr*/ false))
        .Times(1);
  }
  states.acknowledged_bitrate_estimator.IncomingPacketFeedbackVector(
      packet_feedback_vector);
}

#[test]
fn ExpectFastRateChangeWhenLeftAlr() {
  let states= CreateTestStates();
  let packet_feedback_vector= CreateFeedbackVector();
  {
    InSequence dummy;
    EXPECT_CALL(*states.mock_bitrate_estimator,
                Update(packet_feedback_vector[0].receive_time,
                       packet_feedback_vector[0].sent_packet.size,
                       /*in_alr*/ false))
        .Times(1);
    EXPECT_CALL(*states.mock_bitrate_estimator, ExpectFastRateChange())
        .Times(1);
    EXPECT_CALL(*states.mock_bitrate_estimator,
                Update(packet_feedback_vector[1].receive_time,
                       packet_feedback_vector[1].sent_packet.size,
                       /*in_alr*/ false))
        .Times(1);
  }
  states.acknowledged_bitrate_estimator.SetAlrEndedTime(
      Timestamp::Millis(FirstArrivalTimeMs + 1));
  states.acknowledged_bitrate_estimator.IncomingPacketFeedbackVector(
      packet_feedback_vector);
}

#[test]
fn ReturnBitrate() {
  let states= CreateTestStates();
  Option<DataRate> return_value = DataRate::KilobitsPerSec(42);
  EXPECT_CALL(*states.mock_bitrate_estimator, bitrate())
      .Times(1)
      .WillOnce(Return(return_value));
  assert_eq!(return_value, states.acknowledged_bitrate_estimator.bitrate());
}

}  // namespace webrtc*/
