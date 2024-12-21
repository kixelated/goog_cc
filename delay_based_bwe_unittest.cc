/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"

#include <cstdint>

#include "api/transport/bandwidth_usage.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "modules/congestion_controller/goog_cc/delay_based_bwe_unittest_helper.h"
#include "system_wrappers/include/clock.h"
#include "test/gtest.h"



namespace {
const NumProbesCluster0: isize = 5;
const NumProbesCluster1: isize = 8;
const PacedPacketInfo PacingInfo0(0, NumProbesCluster0, 2000);
const PacedPacketInfo PacingInfo1(1, NumProbesCluster1, 4000);
const TargetUtilizationFraction: f32 = 0.95f;
}  // namespace

#[test]
fn ProbeDetection() {
  let now_ms: i64 = self.clock.TimeInMilliseconds();

  // First burst sent at 8 * 1000 / 10 = 800 kbps.
  for (isize i = 0; i < NumProbesCluster0i += 1) {
    self.clock.AdvanceTimeMilliseconds(10);
    now_ms = self.clock.TimeInMilliseconds();
    IncomingFeedback(now_ms, now_ms, 1000, PacingInfo0);
  }
  assert!((self.bitrate_observer.updated());

  // Second burst sent at 8 * 1000 / 5 = 1600 kbps.
  for (isize i = 0; i < NumProbesCluster1i += 1) {
    self.clock.AdvanceTimeMilliseconds(5);
    now_ms = self.clock.TimeInMilliseconds();
    IncomingFeedback(now_ms, now_ms, 1000, PacingInfo1);
  }

  assert!((self.bitrate_observer.updated());
  EXPECT_GT(self.bitrate_observer.latest_bitrate(), 1500000);
}

#[test]
fn ProbeDetectionNonPacedPackets() {
  let now_ms: i64 = self.clock.TimeInMilliseconds();
  // First burst sent at 8 * 1000 / 10 = 800 kbps, but with every other packet
  // not being paced which could mess things up.
  for (isize i = 0; i < NumProbesCluster0i += 1) {
    self.clock.AdvanceTimeMilliseconds(5);
    now_ms = self.clock.TimeInMilliseconds();
    IncomingFeedback(now_ms, now_ms, 1000, PacingInfo0);
    // Non-paced packet, arriving 5 ms after.
    self.clock.AdvanceTimeMilliseconds(5);
    IncomingFeedback(now_ms, now_ms, 100, PacedPacketInfo());
  }

  assert!((self.bitrate_observer.updated());
  EXPECT_GT(self.bitrate_observer.latest_bitrate(), 800000);
}

#[test]
fn ProbeDetectionFasterArrival() {
  let now_ms: i64 = self.clock.TimeInMilliseconds();
  // First burst sent at 8 * 1000 / 10 = 800 kbps.
  // Arriving at 8 * 1000 / 5 = 1600 kbps.
  let send_time_ms: i64 = 0;
  for (isize i = 0; i < NumProbesCluster0i += 1) {
    self.clock.AdvanceTimeMilliseconds(1);
    send_time_ms += 10;
    now_ms = self.clock.TimeInMilliseconds();
    IncomingFeedback(now_ms, send_time_ms, 1000, PacingInfo0);
  }

  assert!(!(self.bitrate_observer.updated());
}

#[test]
fn ProbeDetectionSlowerArrival() {
  let now_ms: i64 = self.clock.TimeInMilliseconds();
  // First burst sent at 8 * 1000 / 5 = 1600 kbps.
  // Arriving at 8 * 1000 / 7 = 1142 kbps.
  // Since the receive rate is significantly below the send rate, we expect to
  // use 95% of the estimated capacity.
  let send_time_ms: i64 = 0;
  for (isize i = 0; i < NumProbesCluster1i += 1) {
    self.clock.AdvanceTimeMilliseconds(7);
    send_time_ms += 5;
    now_ms = self.clock.TimeInMilliseconds();
    IncomingFeedback(now_ms, send_time_ms, 1000, PacingInfo1);
  }

  assert!((self.bitrate_observer.updated());
  assert_relative_eq!(self.bitrate_observer.latest_bitrate(),
              TargetUtilizationFraction * 1140000, 10000);
}

#[test]
fn ProbeDetectionSlowerArrivalHighBitrate() {
  let now_ms: i64 = self.clock.TimeInMilliseconds();
  // Burst sent at 8 * 1000 / 1 = 8000 kbps.
  // Arriving at 8 * 1000 / 2 = 4000 kbps.
  // Since the receive rate is significantly below the send rate, we expect to
  // use 95% of the estimated capacity.
  let send_time_ms: i64 = 0;
  for (isize i = 0; i < NumProbesCluster1i += 1) {
    self.clock.AdvanceTimeMilliseconds(2);
    send_time_ms += 1;
    now_ms = self.clock.TimeInMilliseconds();
    IncomingFeedback(now_ms, send_time_ms, 1000, PacingInfo1);
  }

  assert!((self.bitrate_observer.updated());
  assert_relative_eq!(self.bitrate_observer.latest_bitrate(),
              TargetUtilizationFraction * 4000000, 10000);
}

#[test]
fn GetExpectedBwePeriodMs() {
let default_interval = self.bitrate_estimator.GetExpectedBwePeriod();
  EXPECT_GT(default_interval.ms(), 0);
  CapacityDropTestHelper(1, true, 533, 0);
let interval = self.bitrate_estimator.GetExpectedBwePeriod();
  EXPECT_GT(interval.ms(), 0);
  EXPECT_NE(interval.ms(), default_interval.ms());
}

#[test]
fn InitialBehavior() {
  InitialBehaviorTestHelper(730000);
}

#[test]
fn InitializeResult() {
  DelayBasedBwe::Result result;
  assert_eq!(result.delay_detector_state, BandwidthUsage::Normal);
}

#[test]
fn RateIncreaseReordering() {
  RateIncreaseReorderingTestHelper(730000);
}
#[test]
fn RateIncreaseRtpTimestamps() {
  RateIncreaseRtpTimestampsTestHelper(617);
}

#[test]
fn CapacityDropOneStream() {
  CapacityDropTestHelper(1, false, 500, 0);
}

#[test]
fn CapacityDropPosOffsetChange() {
  CapacityDropTestHelper(1, false, 867, 30000);
}

#[test]
fn CapacityDropNegOffsetChange() {
  CapacityDropTestHelper(1, false, 933, -30000);
}

#[test]
fn CapacityDropOneStreamWrap() {
  CapacityDropTestHelper(1, true, 533, 0);
}

#[test]
fn TestTimestampGrouping() {
  TestTimestampGroupingTestHelper();
}

#[test]
fn TestShortTimeoutAndWrap() {
  // Simulate a client leaving and rejoining the call after 35 seconds. This
  // will make abs send time wrap, so if streams aren't timed out properly
  // the next 30 seconds of packets will be out of order.
  TestWrappingHelper(35);
}

#[test]
fn TestLongTimeoutAndWrap() {
  // Simulate a client leaving and rejoining the call after some multiple of
  // 64 seconds later. This will cause a zero difference in abs send times due
  // to the wrap, but a big difference in arrival time, if streams aren't
  // properly timed out.
  TestWrappingHelper(10 * 64);
}

#[test]
fn TestInitialOveruse() {
  const StartBitrate: DataRate = DataRate::KilobitsPerSec(300);
  const InitialCapacity: DataRate = DataRate::KilobitsPerSec(200);
  const DummySsrc: u32 = 0;
  // High FPS to ensure that we send a lot of packets in a short time.
  const Fps: isize = 90;

  self.stream_generator.AddStream(new test::RtpStream(Fps, StartBitrate.bps()));
  self.stream_generator.set_capacity_bps(InitialCapacity.bps());

  // Needed to initialize the AimdRateControl.
  self.bitrate_estimator.SetStartBitrate(StartBitrate);

  // Produce 40 frames (in 1/3 second) and give them to the estimator.
  let bitrate_bps: i64 = StartBitrate.bps();
  let seen_overuse: bool = false;
  for (isize i = 0; i < 40i += 1) {
    let overuse: bool = GenerateAndProcessFrame(DummySsrc, bitrate_bps);
    if (overuse) {
      assert!((self.bitrate_observer.updated());
      EXPECT_LE(self.bitrate_observer.latest_bitrate(), InitialCapacity.bps());
      EXPECT_GT(self.bitrate_observer.latest_bitrate(),
                0.8 * InitialCapacity.bps());
      bitrate_bps = self.bitrate_observer.latest_bitrate();
      seen_overuse = true;
      break;
    } else if (self.bitrate_observer.updated()) {
      bitrate_bps = self.bitrate_observer.latest_bitrate();
      self.bitrate_observer.Reset();
    }
  }
  assert!((seen_overuse);
  EXPECT_LE(self.bitrate_observer.latest_bitrate(), InitialCapacity.bps());
  EXPECT_GT(self.bitrate_observer.latest_bitrate(), 0.8 * InitialCapacity.bps());
}

#[test]
fn TestTimestampPrecisionHandling() {
  // This test does some basic checks to make sure that timestamps with higher
  // than millisecond precision are handled properly and do not cause any
  // problems in the estimator. Specifically, previously reported in
  // webrtc:14023 and described in more details there, the rounding to the
  // nearest milliseconds caused discrepancy in the accumulated delay. This lead
  // to false-positive overuse detection.
  // Technical details of the test:
  // Send times(ms): 0.000,  9.725, 20.000, 29.725, 40.000, 49.725, ...
  // Recv times(ms): 0.500, 10.000, 20.500, 30.000, 40.500, 50.000, ...
  // Send deltas(ms):   9.750,  10.250,  9.750, 10.250,  9.750, ...
  // Recv deltas(ms):   9.500,  10.500,  9.500, 10.500,  9.500, ...
  // There is no delay building up between the send times and the receive times,
  // therefore this case should never lead to an overuse detection. However, if
  // the time deltas were accidentally rounded to the nearest milliseconds, then
  // all the send deltas would be equal to 10ms while some recv deltas would
  // round up to 11ms which would lead in a false illusion of delay build up.
  let last_bitrate: u32 = self.bitrate_observer.latest_bitrate();
  for (isize i = 0; i < 1000i += 1) {
    self.clock.AdvanceTimeMicroseconds(500);
    IncomingFeedback(self.clock.CurrentTime(),
                     self.clock.CurrentTime() - TimeDelta::Micros(500), 1000,
                     PacedPacketInfo());
    self.clock.AdvanceTimeMicroseconds(9500);
    IncomingFeedback(self.clock.CurrentTime(),
                     self.clock.CurrentTime() - TimeDelta::Micros(250), 1000,
                     PacedPacketInfo());
    self.clock.AdvanceTimeMicroseconds(10000);

    // The bitrate should never decrease in this test.
    EXPECT_LE(last_bitrate, self.bitrate_observer.latest_bitrate());
    last_bitrate = self.bitrate_observer.latest_bitrate();
  }
}

}  // namespace webrtc
