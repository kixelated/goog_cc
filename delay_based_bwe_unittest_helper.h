/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_DELAY_BASED_BWE_UNITTEST_HELPER_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_DELAY_BASED_BWE_UNITTEST_HELPER_H_

#include <stddef.h>
#include <stdint.h>

#include <memory>
#include <vector>

#include "api/transport/field_trial_based_config.h"
#include "api/transport/network_types.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"
#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"
#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"
#include "system_wrappers/include/clock.h"
#include "test/field_trial.h"
#include "test/gtest.h"


namespace test {

pub struct TestBitrateObserver {
 public:
  TestBitrateObserver() : updated_(false), latest_bitrate_(0) {}
  ~TestBitrateObserver() {}

  fn OnReceiveBitrateChanged(u32 bitrate) {
  todo!();
}

fn Reset() { self.updated = false; }

  bool updated() { return self.updated; }

  u32 latest_bitrate() { return self.latest_bitrate; }

 private:
  updated: bool,
  latest_bitrate: u32,
};

pub struct RtpStream {
 public:
  enum { SendSideOffsetUs = 1000000 };

  RtpStream(isize fps, isize bitrate_bps);

  RtpStream(const RtpStream&) = delete;
  RtpStream& operator=(const RtpStream&) = delete;

  // Generates a new frame for this stream. If called too soon after the
  // previous frame, no frame will be generated. The frame is split into
  // packets.
  i64 GenerateFrame(i64 time_now_us,
                        i64* next_sequence_number,
                        Vec<PacketResult>* packets);

  // The send-side time when the next frame can be generated.
  fn next_rtp_time(&self) -> i64 {
todo!();
}

  fn set_bitrate_bps(isize bitrate_bps) {
  todo!();
}

  fn bitrate_bps(&self) -> isize {
todo!();
}

  static bool Compare(const std::unique_ptr<RtpStream>& lhs,
                      const std::unique_ptr<RtpStream>& rhs);

 private:
  fps: isize,
  bitrate_bps: isize,
  next_rtp_time: i64,
};

pub struct StreamGenerator {
 public:
  StreamGenerator(isize capacity, i64 time_now);
  ~StreamGenerator();

  StreamGenerator(const StreamGenerator&) = delete;
  StreamGenerator& operator=(const StreamGenerator&) = delete;

  // Add a new stream.
  fn AddStream(RtpStream* stream) {
  todo!();
}

  // Set the link capacity.
  fn set_capacity_bps(isize capacity_bps) {
  todo!();
}

  // Divides `bitrate_bps` among all streams. The allocated bitrate per stream
  // is decided by the initial allocation ratios.
  fn SetBitrateBps(isize bitrate_bps) {
  todo!();
}

  // Set the RTP timestamp offset for the stream identified by `ssrc`.
  fn set_rtp_timestamp_offset(u32 ssrc, u32 offset) {
  todo!();
}

  // TODO(holmer): Break out the channel simulation part from this class to make
  // it possible to simulate different types of channels.
  i64 GenerateFrame(i64 time_now_us,
                        i64* next_sequence_number,
                        Vec<PacketResult>* packets);

 private:
  // Capacity of the simulated channel in bits per second.
  capacity: isize,
  // The time when the last packet arrived.
  prev_arrival_time_us: i64,
  // All streams being transmitted on this simulated channel.
  streams: Vec<std::unique_ptr<RtpStream>>,
};
}  // namespace test

class DelayBasedBweTest : public ::testing::Test {
 public:
  DelayBasedBweTest();
  ~DelayBasedBweTest() override;

 protected:
  fn AddDefaultStream(&mut self) { todo!(); }

  // Helpers to insert a single packet into the delay-based BWE.
  fn IncomingFeedback(i64 arrival_time_ms,
                        i64 send_time_ms,
                        usize payload_size);
  fn IncomingFeedback(i64 arrival_time_ms,
                        i64 send_time_ms,
                        usize payload_size,
                        const PacedPacketInfo& pacing_info);
  fn IncomingFeedback(receive_time: Timestamp,
                        send_time: Timestamp,
                        usize payload_size,
                        const PacedPacketInfo& pacing_info);

  // Generates a frame of packets belonging to a stream at a given bitrate and
  // with a given ssrc. The stream is pushed through a very simple simulated
  // network, and is then given to the receive-side bandwidth estimator.
  // Returns true if an over-use was seen, false otherwise.
  // The StreamGenerator::updated() should be used to check for any changes in
  // target bitrate after the call to this function.
  bool GenerateAndProcessFrame(u32 ssrc, u32 bitrate_bps);

  // Run the bandwidth estimator with a stream of `number_of_frames` frames, or
  // until it reaches `target_bitrate`.
  // Can for instance be used to run the estimator for some time to get it
  // into a steady state.
  u32 SteadyStateRun(u32 ssrc,
                          isize number_of_frames,
                          u32 start_bitrate,
                          u32 min_bitrate,
                          u32 max_bitrate,
                          u32 target_bitrate);

  fn TestTimestampGroupingTestHelper(&mut self) { todo!(); }

  fn TestWrappingHelper(isize silence_time_s) {
  todo!();
}

  fn InitialBehaviorTestHelper(u32 expected_converge_bitrate) {
  todo!();
}
  fn RateIncreaseReorderingTestHelper(u32 expected_bitrate) {
  todo!();
}
  fn RateIncreaseRtpTimestampsTestHelper(isize expected_iterations) {
  todo!();
}
  fn CapacityDropTestHelper(isize number_of_streams,
                              wrap_time_stamp: bool,
                              u32 expected_bitrate_drop_delta,
                              i64 receiver_clock_offset_change_ms);

  static const u32 DefaultSsrc;
  field_trial_config: FieldTrialBasedConfig,

  std::unique_ptr<test::ScopedFieldTrials>
      field_trial;        // Must be initialized first.
  clock: SimulatedClock,  // Time at the receiver.
  bitrate_observer: test::TestBitrateObserver,
  std::unique_ptr<AcknowledgedBitrateEstimatorInterface>
      self.acknowledged_bitrate_estimator;
  const std::unique_ptr<ProbeBitrateEstimator> self.probe_bitrate_estimator;
  bitrate_estimator: std::unique_ptr<DelayBasedBwe>,
  stream_generator: std::unique_ptr<test::StreamGenerator>,
  arrival_time_offset_ms: i64,
  next_sequence_number: i64,
  first_update: bool,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_DELAY_BASED_BWE_UNITTEST_HELPER_H_
