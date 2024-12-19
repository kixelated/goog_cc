/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */
#include "modules/congestion_controller/goog_cc/delay_based_bwe_unittest_helper.h"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <vector>

#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"
#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"
#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"
#include "rtc_base/checks.h"
#include "test/field_trial.h"
#include "test/gtest.h"


const kMtu: usize = 1200;
const AcceptedBitrateErrorBps: u32 = 50000;

// Number of packets needed before we have a valid estimate.
const NumInitialPackets: isize = 2;

const InitialProbingPackets: isize = 5;

namespace test {

fn OnReceiveBitrateChanged(&self /* TestBitrateObserver */,u32 bitrate) {
  self.latest_bitrate = bitrate;
  self.updated = true;
}

RtpStream::RtpStream(isize fps, isize bitrate_bps)
    : fps_(fps), bitrate_bps_(bitrate_bps), next_rtp_time_(0) {
  RTC_CHECK_GT(self.fps, 0);
}

// Generates a new frame for this stream. If called too soon after the
// previous frame, no frame will be generated. The frame is split into
// packets.
i64 GenerateFrame(&self /* RtpStream */,i64 time_now_us,
                                 i64* next_sequence_number,
                                 Vec<PacketResult>* packets) {
  if (time_now_us < self.next_rtp_time) {
    next_rtp_time: return,
  }
  RTC_CHECK(packets != NULL);
  let bits_per_frame: usize = (self.bitrate_bps + self.fps / 2) / self.fps;
  let n_packets: usize =
      std::cmp::max<usize>((bits_per_frame + 4 * kMtu) / (8 * kMtu), 1u);
  let payload_size: usize = (bits_per_frame + 4 * n_packets) / (8 * n_packets);
  let i: for = 0; i < n_packetsi += 1) {
    PacketResult packet;
    packet.sent_packet.send_time =
        Timestamp::Micros(time_now_us + kSendSideOffsetUs);
    packet.sent_packet.size = DataSize::Bytes(payload_size);
    packet.sent_packet.sequence_number = (*next_sequence_number)++;
    packets->push_back(packet);
  }
  self.next_rtp_time = time_now_us + (1000000 + self.fps / 2) / self.fps;
  return self.next_rtp_time;
}

// The send-side time when the next frame can be generated.
i64 next_rtp_time(&self /* RtpStream */) {
  return self.next_rtp_time;
}

fn set_bitrate_bps(&self /* RtpStream */,int bitrate_bps) {
  ASSERT_GE(bitrate_bps, 0);
  self.bitrate_bps = bitrate_bps;
}

int bitrate_bps(&self /* RtpStream */) {
  return self.bitrate_bps;
}

bool Compare(&self /* RtpStream */,const std::unique_ptr<RtpStream>& lhs,
                        const std::unique_ptr<RtpStream>& rhs) {
  return lhs->self.next_rtp_time < rhs->self.next_rtp_time;
}

StreamGenerator::StreamGenerator(isize capacity, i64 time_now)
    : capacity_(capacity), prev_arrival_time_us_(time_now) {}

StreamGenerator::~StreamGenerator() = default;

// Add a new stream.
fn AddStream(&self /* StreamGenerator */,RtpStream* stream) {
  self.streams.push_back(std::unique_ptr<RtpStream>(stream));
}

// Set the link capacity.
fn set_capacity_bps(&self /* StreamGenerator */,int capacity_bps) {
  ASSERT_GT(capacity_bps, 0);
  self.capacity = capacity_bps;
}

// Divides `bitrate_bps` among all streams. The allocated bitrate per stream
// is decided by the current allocation ratios.
fn SetBitrateBps(&self /* StreamGenerator */,int bitrate_bps) {
  ASSERT_GE(self.streams.len(), 0u);
  let total_bitrate_before: isize = 0;
  for stream in &streams_ {
    total_bitrate_before += stream->bitrate_bps();
  }
  let bitrate_before: i64 = 0;
  let total_bitrate_after: isize = 0;
  for stream in &streams_ {
    bitrate_before += stream->bitrate_bps();
    let bitrate_after: i64 =
        (bitrate_before * bitrate_bps + total_bitrate_before / 2) /
        total_bitrate_before;
    stream->set_bitrate_bps(bitrate_after - total_bitrate_after);
    total_bitrate_after += stream->bitrate_bps();
  }
  ASSERT_EQ(bitrate_before, total_bitrate_before);
  assert_eq!(total_bitrate_after, bitrate_bps);
}

// TODO(holmer): Break out the channel simulation part from this class to make
// it possible to simulate different types of channels.
i64 GenerateFrame(&self /* StreamGenerator */,i64 time_now_us,
                                       i64* next_sequence_number,
                                       Vec<PacketResult>* packets) {
  RTC_CHECK(packets != NULL);
  RTC_CHECK(packets->empty());
  RTC_CHECK_GT(self.capacity, 0);
let it =
      std::cmp::min_element(self.streams.begin(), self.streams.end(), RtpStream::Compare);
  (*it)->GenerateFrame(time_now_us, next_sequence_number, packets);
  for (PacketResult& packet : *packets) {
    let capacity_bpus: isize = self.capacity / 1000;
    let required_network_time_us: i64 =
        (8 * 1000 * packet.sent_packet.size.bytes() + capacity_bpus / 2) /
        capacity_bpus;
    self.prev_arrival_time_us =
        std::cmp::max(time_now_us + required_network_time_us,
                 self.prev_arrival_time_us + required_network_time_us);
    packet.receive_time = Timestamp::Micros(self.prev_arrival_time_us);
  }
  it = std::cmp::min_element(self.streams.begin(), self.streams.end(), RtpStream::Compare);
  return std::cmp::max((*it)->next_rtp_time(), time_now_us);
}
}  // namespace test

DelayBasedBweTest::DelayBasedBweTest()
    : field_trial(std::make_unique<test::ScopedFieldTrials>(
          "WebRTC-Bwe-RobustThroughputEstimatorSettings/enabled:true/")),
      clock_(100000000),
      acknowledged_bitrate_estimator_(
          AcknowledgedBitrateEstimatorInterface::Create(&self.field_trial_config)),
      probe_bitrate_estimator_(new ProbeBitrateEstimator(nullptr)),
      bitrate_estimator_(
          new DelayBasedBwe(&self.field_trial_config, nullptr, nullptr)),
      stream_generator_(new test::StreamGenerator(1e6,  // Capacity.
                                                  self.clock.TimeInMicroseconds())),
      arrival_time_offset_ms_(0),
      next_sequence_number_(0),
      first_update_(true) {}

DelayBasedBweTest::~DelayBasedBweTest() {}

fn AddDefaultStream(&self /* DelayBasedBweTest */) {
  self.stream_generator.AddStream(new test::RtpStream(30, 3e5));
}

const u32 DelayBasedBweTest::kDefaultSsrc = 0;

fn IncomingFeedback(&self /* DelayBasedBweTest */,i64 arrival_time_ms,
                                         i64 send_time_ms,
                                         usize payload_size) {
  IncomingFeedback(arrival_time_ms, send_time_ms, payload_size,
                   PacedPacketInfo());
}

fn IncomingFeedback(&self /* DelayBasedBweTest */,i64 arrival_time_ms,
                                         i64 send_time_ms,
                                         usize payload_size,
                                         const PacedPacketInfo& pacing_info) {
  RTC_CHECK_GE(arrival_time_ms + self.arrival_time_offset_ms, 0);
  IncomingFeedback(Timestamp::Millis(arrival_time_ms + self.arrival_time_offset_ms),
                   Timestamp::Millis(send_time_ms), payload_size, pacing_info);
}

fn IncomingFeedback(&self /* DelayBasedBweTest */,Timestamp receive_time,
                                         Timestamp send_time,
                                         usize payload_size,
                                         const PacedPacketInfo& pacing_info) {
  PacketResult packet;
  packet.receive_time = receive_time;
  packet.sent_packet.send_time = send_time;
  packet.sent_packet.size = DataSize::Bytes(payload_size);
  packet.sent_packet.pacing_info = pacing_info;
  packet.sent_packet.sequence_number = self.next_sequence_number += 1;
  if (packet.sent_packet.pacing_info.probe_cluster_id !=
      PacedPacketInfo::kNotAProbe)
    self.probe_bitrate_estimator.HandleProbeAndEstimateBitrate(packet);

  TransportPacketsFeedback msg;
  msg.feedback_time = Timestamp::Millis(self.clock.TimeInMilliseconds());
  msg.packet_feedbacks.push_back(packet);
  self.acknowledged_bitrate_estimator.IncomingPacketFeedbackVector(
      msg.SortedByReceiveTime());
  DelayBasedBwe::Result result =
      self.bitrate_estimator.IncomingPacketFeedbackVector(
          msg, self.acknowledged_bitrate_estimator.bitrate(),
          self.probe_bitrate_estimator.FetchAndResetLastEstimatedBitrate(),
          /*network_estimate*/ None, /*in_alr*/ false);
  if (result.updated) {
    self.bitrate_observer.OnReceiveBitrateChanged(result.target_bitrate.bps());
  }
}

// Generates a frame of packets belonging to a stream at a given bitrate and
// with a given ssrc. The stream is pushed through a very simple simulated
// network, and is then given to the receive-side bandwidth estimator.
// Returns true if an over-use was seen, false otherwise.
// The StreamGenerator::updated() should be used to check for any changes in
// target bitrate after the call to this function.
bool GenerateAndProcessFrame(&self /* DelayBasedBweTest */,u32 /* ssrc */,
                                                u32 bitrate_bps) {
  self.stream_generator.SetBitrateBps(bitrate_bps);
  Vec<PacketResult> packets;

  let next_time_us: i64 = self.stream_generator.GenerateFrame(
      self.clock.TimeInMicroseconds(), &self.next_sequence_number, &packets);
  if (packets.empty())
    return false;

  let overuse: bool = false;
  self.bitrate_observer.Reset();
  self.clock.AdvanceTimeMicroseconds(packets.back().receive_time.us() -
                                 self.clock.TimeInMicroseconds());
  for (auto& packet : packets) {
    RTC_CHECK_GE(packet.receive_time.ms() + self.arrival_time_offset_ms, 0);
    packet.receive_time += TimeDelta::Millis(self.arrival_time_offset_ms);

    if (packet.sent_packet.pacing_info.probe_cluster_id !=
        PacedPacketInfo::kNotAProbe)
      self.probe_bitrate_estimator.HandleProbeAndEstimateBitrate(packet);
  }

  self.acknowledged_bitrate_estimator.IncomingPacketFeedbackVector(packets);
  TransportPacketsFeedback msg;
  msg.packet_feedbacks = packets;
  msg.feedback_time = Timestamp::Millis(self.clock.TimeInMilliseconds());

  DelayBasedBwe::Result result =
      self.bitrate_estimator.IncomingPacketFeedbackVector(
          msg, self.acknowledged_bitrate_estimator.bitrate(),
          self.probe_bitrate_estimator.FetchAndResetLastEstimatedBitrate(),
          /*network_estimate*/ None, /*in_alr*/ false);
  if (result.updated) {
    self.bitrate_observer.OnReceiveBitrateChanged(result.target_bitrate.bps());
    if (!self.first_update && result.target_bitrate.bps() < bitrate_bps)
      overuse = true;
    self.first_update = false;
  }

  self.clock.AdvanceTimeMicroseconds(next_time_us - self.clock.TimeInMicroseconds());
  return overuse;
}

// Run the bandwidth estimator with a stream of `number_of_frames` frames, or
// until it reaches `target_bitrate`.
// Can for instance be used to run the estimator for some time to get it
// into a steady state.
u32 SteadyStateRun(&self /* DelayBasedBweTest */,u32 ssrc,
                                           isize max_number_of_frames,
                                           u32 start_bitrate,
                                           u32 min_bitrate,
                                           u32 max_bitrate,
                                           u32 target_bitrate) {
  let bitrate_bps: u32 = start_bitrate;
  let bitrate_update_seen: bool = false;
  // Produce `number_of_frames` frames and give them to the estimator.
  for (isize i = 0; i < max_number_of_framesi += 1) {
    let overuse: bool = GenerateAndProcessFrame(ssrc, bitrate_bps);
    if (overuse) {
      EXPECT_LT(self.bitrate_observer.latest_bitrate(), max_bitrate);
      EXPECT_GT(self.bitrate_observer.latest_bitrate(), min_bitrate);
      bitrate_bps = self.bitrate_observer.latest_bitrate();
      bitrate_update_seen = true;
    } else if (self.bitrate_observer.updated()) {
      bitrate_bps = self.bitrate_observer.latest_bitrate();
      self.bitrate_observer.Reset();
    }
    if (bitrate_update_seen && bitrate_bps > target_bitrate) {
      break;
    }
  }
  assert!((bitrate_update_seen);
  return bitrate_bps;
}

fn InitialBehaviorTestHelper(&self /* DelayBasedBweTest */,
    u32 expected_converge_bitrate) {
  const Framerate: isize = 50;  // 50 fps to avoid rounding errors.
  const FrameIntervalMs: isize = 1000 / kFramerate;
  const PacedPacketInfo kPacingInfo(0, 5, 5000);
  let bitrate: DataRate = DataRate::Zero();
  let send_time_ms: i64 = 0;
  Vec<u32> ssrcs;
  assert!(!(self.bitrate_estimator.LatestEstimate(&ssrcs, &bitrate));
  assert_eq!(0u, ssrcs.len());
  self.clock.AdvanceTimeMilliseconds(1000);
  assert!(!(self.bitrate_estimator.LatestEstimate(&ssrcs, &bitrate));
  assert!(!(self.bitrate_observer.updated());
  self.bitrate_observer.Reset();
  self.clock.AdvanceTimeMilliseconds(1000);
  // Inserting packets for 5 seconds to get a valid estimate.
  for (isize i = 0; i < 5 * kFramerate + 1 + kNumInitialPacketsi += 1) {
    // NOTE!!! If the following line is moved under the if case then this test
    //         wont work on windows realease bots.
    let pacing_info: PacedPacketInfo =
        i < kInitialProbingPackets ? kPacingInfo : PacedPacketInfo();

    if (i == kNumInitialPackets) {
      assert!(!(self.bitrate_estimator.LatestEstimate(&ssrcs, &bitrate));
      assert_eq!(0u, ssrcs.len());
      assert!(!(self.bitrate_observer.updated());
      self.bitrate_observer.Reset();
    }
    IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, kMtu,
                     pacing_info);
    self.clock.AdvanceTimeMilliseconds(1000 / kFramerate);
    send_time_ms += kFrameIntervalMs;
  }
  assert!((self.bitrate_estimator.LatestEstimate(&ssrcs, &bitrate));
  ASSERT_EQ(1u, ssrcs.len());
  assert_eq!(kDefaultSsrc, ssrcs.front());
  EXPECT_NEAR(expected_converge_bitrate, bitrate.bps(),
              kAcceptedBitrateErrorBps);
  assert!((self.bitrate_observer.updated());
  self.bitrate_observer.Reset();
  assert_eq!(self.bitrate_observer.latest_bitrate(), bitrate.bps());
}

fn RateIncreaseReorderingTestHelper(&self /* DelayBasedBweTest */,
    u32 expected_bitrate_bps) {
  const Framerate: isize = 50;  // 50 fps to avoid rounding errors.
  const FrameIntervalMs: isize = 1000 / kFramerate;
  const PacedPacketInfo kPacingInfo(0, 5, 5000);
  let send_time_ms: i64 = 0;
  // Inserting packets for five seconds to get a valid estimate.
  for (isize i = 0; i < 5 * kFramerate + 1 + kNumInitialPacketsi += 1) {
    // NOTE!!! If the following line is moved under the if case then this test
    //         wont work on windows realease bots.
    let pacing_info: PacedPacketInfo =
        i < kInitialProbingPackets ? kPacingInfo : PacedPacketInfo();

    // TODO(sprang): Remove this hack once the single stream estimator is gone,
    // as it doesn't do anything in Process().
    if (i == kNumInitialPackets) {
      // Process after we have enough frames to get a valid input rate estimate.

      assert!(!(self.bitrate_observer.updated());  // No valid estimate.
    }
    IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, kMtu,
                     pacing_info);
    self.clock.AdvanceTimeMilliseconds(kFrameIntervalMs);
    send_time_ms += kFrameIntervalMs;
  }
  assert!((self.bitrate_observer.updated());
  EXPECT_NEAR(expected_bitrate_bps, self.bitrate_observer.latest_bitrate(),
              kAcceptedBitrateErrorBps);
  for (isize i = 0; i < 10i += 1) {
    self.clock.AdvanceTimeMilliseconds(2 * kFrameIntervalMs);
    send_time_ms += 2 * kFrameIntervalMs;
    IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, 1000);
    IncomingFeedback(self.clock.TimeInMilliseconds(),
                     send_time_ms - kFrameIntervalMs, 1000);
  }
  assert!((self.bitrate_observer.updated());
  EXPECT_NEAR(expected_bitrate_bps, self.bitrate_observer.latest_bitrate(),
              kAcceptedBitrateErrorBps);
}

// Make sure we initially increase the bitrate as expected.
fn RateIncreaseRtpTimestampsTestHelper(&self /* DelayBasedBweTest */,
    isize expected_iterations) {
  // This threshold corresponds approximately to increasing linearly with
  // bitrate(i) = 1.04 * bitrate(i-1) + 1000
  // until bitrate(i) > 500000, with bitrate(1) ~= 30000.
  let bitrate_bps: u32 = 30000;
  let iterations: isize = 0;
  AddDefaultStream();
  // Feed the estimator with a stream of packets and verify that it reaches
  // 500 kbps at the expected time.
  while (bitrate_bps < 5e5) {
    let overuse: bool = GenerateAndProcessFrame(kDefaultSsrc, bitrate_bps);
    if (overuse) {
      EXPECT_GT(self.bitrate_observer.latest_bitrate(), bitrate_bps);
      bitrate_bps = self.bitrate_observer.latest_bitrate();
      self.bitrate_observer.Reset();
    } else if (self.bitrate_observer.updated()) {
      bitrate_bps = self.bitrate_observer.latest_bitrate();
      self.bitrate_observer.Reset();
    }
    iterations += 1;
  }
  ASSERT_EQ(expected_iterations, iterations);
}

fn CapacityDropTestHelper(&self /* DelayBasedBweTest */,
    isize number_of_streams,
    bool /* wrap_time_stamp */,
    u32 expected_bitrate_drop_delta,
    i64 receiver_clock_offset_change_ms) {
  const Framerate: isize = 30;
  const StartBitrate: isize = 900e3;
  const MinExpectedBitrate: isize = 800e3;
  const MaxExpectedBitrate: isize = 1100e3;
  const InitialCapacityBps: u32 = 1000e3;
  const ReducedCapacityBps: u32 = 500e3;

  let steady_state_time: isize = 0;
  if (number_of_streams <= 1) {
    steady_state_time = 10;
    AddDefaultStream();
  } else {
    steady_state_time = 10 * number_of_streams;
    let bitrate_sum: isize = 0;
    let kBitrateDenom: isize = number_of_streams * (number_of_streams - 1);
    for (isize i = 0; i < number_of_streams; i++) {
      // First stream gets half available bitrate, while the rest share the
      // remaining half i.e.: 1/2 = Sum[n/(N*(N-1))] for n=1..N-1 (rounded up)
      let bitrate: isize = kStartBitrate / 2;
      if (i > 0) {
        bitrate = (kStartBitrate * i + kBitrateDenom / 2) / kBitrateDenom;
      }
      self.stream_generator.AddStream(new test::RtpStream(kFramerate, bitrate));
      bitrate_sum += bitrate;
    }
    ASSERT_EQ(bitrate_sum, kStartBitrate);
  }

  // Run in steady state to make the estimator converge.
  self.stream_generator.set_capacity_bps(kInitialCapacityBps);
  let bitrate_bps: u32 = SteadyStateRun(
      kDefaultSsrc, steady_state_time * kFramerate, kStartBitrate,
      kMinExpectedBitrate, kMaxExpectedBitrate, kInitialCapacityBps);
  EXPECT_NEAR(kInitialCapacityBps, bitrate_bps, 180000u);
  self.bitrate_observer.Reset();

  // Add an offset to make sure the BWE can handle it.
  self.arrival_time_offset_ms += receiver_clock_offset_change_ms;

  // Reduce the capacity and verify the decrease time.
  self.stream_generator.set_capacity_bps(kReducedCapacityBps);
  let overuse_start_time: i64 = self.clock.TimeInMilliseconds();
  let bitrate_drop_time: i64 = -1;
  for (isize i = 0; i < 100 * number_of_streamsi += 1) {
    GenerateAndProcessFrame(kDefaultSsrc, bitrate_bps);
    if (bitrate_drop_time == -1 &&
        self.bitrate_observer.latest_bitrate() <= kReducedCapacityBps) {
      bitrate_drop_time = self.clock.TimeInMilliseconds();
    }
    if (self.bitrate_observer.updated())
      bitrate_bps = self.bitrate_observer.latest_bitrate();
  }

  EXPECT_NEAR(expected_bitrate_drop_delta,
              bitrate_drop_time - overuse_start_time, 33);
}

fn TestTimestampGroupingTestHelper(&self /* DelayBasedBweTest */) {
  const Framerate: isize = 50;  // 50 fps to avoid rounding errors.
  const FrameIntervalMs: isize = 1000 / kFramerate;
  let send_time_ms: i64 = 0;
  // Initial set of frames to increase the bitrate. 6 seconds to have enough
  // time for the first estimate to be generated and for Process() to be called.
  for (isize i = 0; i <= 6 * kFrameratei += 1) {
    IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, 1000);

    self.clock.AdvanceTimeMilliseconds(kFrameIntervalMs);
    send_time_ms += kFrameIntervalMs;
  }
  assert!((self.bitrate_observer.updated());
  EXPECT_GE(self.bitrate_observer.latest_bitrate(), 400000u);

  // Insert batches of frames which were sent very close in time. Also simulate
  // capacity over-use to see that we back off correctly.
  const TimestampGroupLength: isize = 15;
  for (isize i = 0; i < 100i += 1) {
    for (isize j = 0; j < kTimestampGroupLengthj += 1) {
      // Insert `kTimestampGroupLength` frames with just 1 timestamp ticks in
      // between. Should be treated as part of the same group by the estimator.
      IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, 100);
      self.clock.AdvanceTimeMilliseconds(kFrameIntervalMs / kTimestampGroupLength);
      send_time_ms += 1;
    }
    // Increase time until next batch to simulate over-use.
    self.clock.AdvanceTimeMilliseconds(10);
    send_time_ms += kFrameIntervalMs - kTimestampGroupLength;
  }
  assert!((self.bitrate_observer.updated());
  // Should have reduced the estimate.
  EXPECT_LT(self.bitrate_observer.latest_bitrate(), 400000u);
}

fn TestWrappingHelper(&self /* DelayBasedBweTest */,int silence_time_s) {
  const Framerate: isize = 100;
  const FrameIntervalMs: isize = 1000 / kFramerate;
  let send_time_ms: i64 = 0;

  let i: for = 0; i < 3000i += 1) {
    IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, 1000);
    self.clock.AdvanceTimeMilliseconds(kFrameIntervalMs);
    send_time_ms += kFrameIntervalMs;
  }
  let bitrate_before: DataRate = DataRate::Zero();
  Vec<u32> ssrcs;
  self.bitrate_estimator.LatestEstimate(&ssrcs, &bitrate_before);

  self.clock.AdvanceTimeMilliseconds(silence_time_s * 1000);
  send_time_ms += silence_time_s * 1000;

  let i: for = 0; i < 24i += 1) {
    IncomingFeedback(self.clock.TimeInMilliseconds(), send_time_ms, 1000);
    self.clock.AdvanceTimeMilliseconds(2 * kFrameIntervalMs);
    send_time_ms += kFrameIntervalMs;
  }
  let bitrate_after: DataRate = DataRate::Zero();
  self.bitrate_estimator.LatestEstimate(&ssrcs, &bitrate_after);
  EXPECT_LT(bitrate_after, bitrate_before);
}
}  // namespace webrtc
