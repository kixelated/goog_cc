/*
 *  Copyright (c) 2014 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/send_side_bandwidth_estimation.h"

#include <cstdint>

#include "api/rtc_event_log/rtc_event.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/events/rtc_event_bwe_update_loss_based.h"
#include "logging/rtc_event_log/mock/mock_rtc_event_log.h"
#include "test/explicit_key_value_config.h"
#include "test/gmock.h"
#include "test/gtest.h"



MATCHER(LossBasedBweUpdateWithBitrateOnly, "") {
  if (arg->GetType() != RtcEvent::Type::BweUpdateLossBased) {
    return false;
  }
let bwe_event = static_cast<RtcEventBweUpdateLossBased*>(arg);
  return bwe_event->bitrate_bps() > 0 && bwe_event->fraction_loss() == 0;
}

MATCHER(LossBasedBweUpdateWithBitrateAndLossFraction, "") {
  if (arg->GetType() != RtcEvent::Type::BweUpdateLossBased) {
    return false;
  }
let bwe_event = static_cast<RtcEventBweUpdateLossBased*>(arg);
  return bwe_event->bitrate_bps() > 0 && bwe_event->fraction_loss() > 0;
}

fn TestProbing(bool use_delay_based) {
  ::testing::NiceMock<MockRtcEventLog> event_log;
  test::ExplicitKeyValueConfig key_value_config("");
  SendSideBandwidthEstimation bwe(&key_value_config, &event_log);
  let now_ms: i64 = 0;
  bwe.SetMinMaxBitrate(DataRate::BitsPerSec(100000),
                       DataRate::BitsPerSec(1500000));
  bwe.SetSendBitrate(DataRate::BitsPerSec(200000), Timestamp::Millis(now_ms));

  const RembBps: isize = 1000000;
  const SecondRembBps: isize = kRembBps + 500000;

  bwe.UpdatePacketsLost(/*packets_lost=*/0, /*number_of_packets=*/1,
                        Timestamp::Millis(now_ms));
  bwe.UpdateRtt(TimeDelta::Millis(50), Timestamp::Millis(now_ms));

  // Initial REMB applies immediately.
  if (use_delay_based) {
    bwe.UpdateDelayBasedEstimate(Timestamp::Millis(now_ms),
                                 DataRate::BitsPerSec(kRembBps));
  } else {
    bwe.UpdateReceiverEstimate(Timestamp::Millis(now_ms),
                               DataRate::BitsPerSec(kRembBps));
  }
  bwe.UpdateEstimate(Timestamp::Millis(now_ms));
  assert_eq!(kRembBps, bwe.target_rate().bps());

  // Second REMB doesn't apply immediately.
  now_ms += 2001;
  if (use_delay_based) {
    bwe.UpdateDelayBasedEstimate(Timestamp::Millis(now_ms),
                                 DataRate::BitsPerSec(kSecondRembBps));
  } else {
    bwe.UpdateReceiverEstimate(Timestamp::Millis(now_ms),
                               DataRate::BitsPerSec(kSecondRembBps));
  }
  bwe.UpdateEstimate(Timestamp::Millis(now_ms));
  assert_eq!(kRembBps, bwe.target_rate().bps());
}

#[test]
fn InitialRembWithProbing() {
  TestProbing(false);
}

#[test]
fn InitialDelayBasedBweWithProbing() {
  TestProbing(true);
}

#[test]
fn DoesntReapplyBitrateDecreaseWithoutFollowingRemb() {
  MockRtcEventLog event_log;
  EXPECT_CALL(event_log, LogProxy(LossBasedBweUpdateWithBitrateOnly()))
      .Times(1);
  EXPECT_CALL(event_log,
              LogProxy(LossBasedBweUpdateWithBitrateAndLossFraction()))
      .Times(1);
  test::ExplicitKeyValueConfig key_value_config("");
  SendSideBandwidthEstimation bwe(&key_value_config, &event_log);
  static const MinBitrateBps: isize = 100000;
  static const InitialBitrateBps: isize = 1000000;
  let now_ms: i64 = 1000;
  bwe.SetMinMaxBitrate(DataRate::BitsPerSec(kMinBitrateBps),
                       DataRate::BitsPerSec(1500000));
  bwe.SetSendBitrate(DataRate::BitsPerSec(kInitialBitrateBps),
                     Timestamp::Millis(now_ms));

  static const FractionLoss: uint8_t = 128;
  static const RttMs: i64 = 50;
  now_ms += 10000;

  assert_eq!(kInitialBitrateBps, bwe.target_rate().bps());
  assert_eq!(0, bwe.fraction_loss());
  assert_eq!(0, bwe.round_trip_time().ms());

  // Signal heavy loss to go down in bitrate.
  bwe.UpdatePacketsLost(/*packets_lost=*/50, /*number_of_packets=*/100,
                        Timestamp::Millis(now_ms));
  bwe.UpdateRtt(TimeDelta::Millis(kRttMs), Timestamp::Millis(now_ms));

  // Trigger an update 2 seconds later to not be rate limited.
  now_ms += 1000;
  bwe.UpdateEstimate(Timestamp::Millis(now_ms));
  EXPECT_LT(bwe.target_rate().bps(), kInitialBitrateBps);
  // Verify that the obtained bitrate isn't hitting the min bitrate, or this
  // test doesn't make sense. If this ever happens, update the thresholds or
  // loss rates so that it doesn't hit min bitrate after one bitrate update.
  EXPECT_GT(bwe.target_rate().bps(), kMinBitrateBps);
  assert_eq!(kFractionLoss, bwe.fraction_loss());
  assert_eq!(kRttMs, bwe.round_trip_time().ms());

  // Triggering an update shouldn't apply further downgrade nor upgrade since
  // there's no intermediate receiver block received indicating whether this is
  // currently good or not.
  let last_bitrate_bps: isize = bwe.target_rate().bps();
  // Trigger an update 2 seconds later to not be rate limited (but it still
  // shouldn't update).
  now_ms += 1000;
  bwe.UpdateEstimate(Timestamp::Millis(now_ms));

  assert_eq!(last_bitrate_bps, bwe.target_rate().bps());
  // The old loss rate should still be applied though.
  assert_eq!(kFractionLoss, bwe.fraction_loss());
  assert_eq!(kRttMs, bwe.round_trip_time().ms());
}

#[test]
fn SettingSendBitrateOverridesDelayBasedEstimate() {
  ::testing::NiceMock<MockRtcEventLog> event_log;
  test::ExplicitKeyValueConfig key_value_config("");
  SendSideBandwidthEstimation bwe(&key_value_config, &event_log);
  static const MinBitrateBps: isize = 10000;
  static const MaxBitrateBps: isize = 10000000;
  static const InitialBitrateBps: isize = 300000;
  static const DelayBasedBitrateBps: isize = 350000;
  static const ForcedHighBitrate: isize = 2500000;

  let now_ms: i64 = 0;

  bwe.SetMinMaxBitrate(DataRate::BitsPerSec(kMinBitrateBps),
                       DataRate::BitsPerSec(kMaxBitrateBps));
  bwe.SetSendBitrate(DataRate::BitsPerSec(kInitialBitrateBps),
                     Timestamp::Millis(now_ms));

  bwe.UpdateDelayBasedEstimate(Timestamp::Millis(now_ms),
                               DataRate::BitsPerSec(kDelayBasedBitrateBps));
  bwe.UpdateEstimate(Timestamp::Millis(now_ms));
  EXPECT_GE(bwe.target_rate().bps(), kInitialBitrateBps);
  EXPECT_LE(bwe.target_rate().bps(), kDelayBasedBitrateBps);

  bwe.SetSendBitrate(DataRate::BitsPerSec(kForcedHighBitrate),
                     Timestamp::Millis(now_ms));
  assert_eq!(bwe.target_rate().bps(), kForcedHighBitrate);
}

#[test]
fn DefaultEnabled() {
  test::ExplicitKeyValueConfig key_value_config("");
  RttBasedBackoff rtt_backoff(&key_value_config);
  assert!((rtt_backoff.self.rtt_limit.IsFinite());
}

#[test]
fn CanBeDisabled() {
  test::ExplicitKeyValueConfig key_value_config(
      "WebRTC-Bwe-MaxRttLimit/Disabled/");
  RttBasedBackoff rtt_backoff(&key_value_config);
  assert!((rtt_backoff.self.rtt_limit.IsPlusInfinity());
}

#[test]
fn FractionLossIsNotOverflowed() {
  MockRtcEventLog event_log;
  test::ExplicitKeyValueConfig key_value_config("");
  SendSideBandwidthEstimation bwe(&key_value_config, &event_log);
  static const MinBitrateBps: isize = 100000;
  static const InitialBitrateBps: isize = 1000000;
  let now_ms: i64 = 1000;
  bwe.SetMinMaxBitrate(DataRate::BitsPerSec(kMinBitrateBps),
                       DataRate::BitsPerSec(1500000));
  bwe.SetSendBitrate(DataRate::BitsPerSec(kInitialBitrateBps),
                     Timestamp::Millis(now_ms));

  now_ms += 10000;

  assert_eq!(kInitialBitrateBps, bwe.target_rate().bps());
  assert_eq!(0, bwe.fraction_loss());

  // Signal negative loss.
  bwe.UpdatePacketsLost(/*packets_lost=*/-1, /*number_of_packets=*/100,
                        Timestamp::Millis(now_ms));
  assert_eq!(0, bwe.fraction_loss());
}

#[test]
fn RttIsAboveLimitIfRttGreaterThanLimit() {
  ::testing::NiceMock<MockRtcEventLog> event_log;
  test::ExplicitKeyValueConfig key_value_config("");
  SendSideBandwidthEstimation bwe(&key_value_config, &event_log);
  static const MinBitrateBps: isize = 10000;
  static const MaxBitrateBps: isize = 10000000;
  static const InitialBitrateBps: isize = 300000;
  let now_ms: i64 = 0;
  bwe.SetMinMaxBitrate(DataRate::BitsPerSec(kMinBitrateBps),
                       DataRate::BitsPerSec(kMaxBitrateBps));
  bwe.SetSendBitrate(DataRate::BitsPerSec(kInitialBitrateBps),
                     Timestamp::Millis(now_ms));
  bwe.UpdatePropagationRtt(/*at_time=*/Timestamp::Millis(now_ms),
                           /*propagation_rtt=*/TimeDelta::Millis(5000));
  assert!((bwe.IsRttAboveLimit());
}

#[test]
fn RttIsBelowLimitIfRttLessThanLimit() {
  ::testing::NiceMock<MockRtcEventLog> event_log;
  test::ExplicitKeyValueConfig key_value_config("");
  SendSideBandwidthEstimation bwe(&key_value_config, &event_log);
  static const MinBitrateBps: isize = 10000;
  static const MaxBitrateBps: isize = 10000000;
  static const InitialBitrateBps: isize = 300000;
  let now_ms: i64 = 0;
  bwe.SetMinMaxBitrate(DataRate::BitsPerSec(kMinBitrateBps),
                       DataRate::BitsPerSec(kMaxBitrateBps));
  bwe.SetSendBitrate(DataRate::BitsPerSec(kInitialBitrateBps),
                     Timestamp::Millis(now_ms));
  bwe.UpdatePropagationRtt(/*at_time=*/Timestamp::Millis(now_ms),
                           /*propagation_rtt=*/TimeDelta::Millis(1000));
  assert!(!(bwe.IsRttAboveLimit());
}

}  // namespace webrtc
