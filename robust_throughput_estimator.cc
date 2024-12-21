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

#include <algorithm>
#include <optional>
#include <utility>
#include <vector>

#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"
#include "rtc_base/checks.h"
#include "rtc_base/logging.h"



RobustThroughputEstimator::RobustThroughputEstimator(
    const RobustThroughputEstimatorSettings& settings)
    : settings_(settings),
      latest_discarded_send_time_(Timestamp::MinusInfinity()) {
  assert!(settings.enabled);
}

RobustThroughputEstimator::~RobustThroughputEstimator() {}

bool FirstPacketOutsideWindow(&self /* RobustThroughputEstimator */) {
  if (self.window.is_empty())
    return false;
  if (self.window.len() > self.settings.max_window_packets)
    return true;
  let current_window_duration: TimeDelta =
      self.window.back().receive_time - self.window.front().receive_time;
  if (current_window_duration > self.settings.max_window_duration)
    return true;
  if (self.window.len() > self.settings.window_packets &&
      current_window_duration > self.settings.min_window_duration) {
    return true;
  }
  return false;
}

fn IncomingPacketFeedbackVector(&self /* RobustThroughputEstimator */,
    packet_feedback_vector: &[PacketResult]) {
  assert!(std::is_sorted(packet_feedback_vector.begin(),
                            packet_feedback_vector.end(),
                            PacketResult::ReceiveTimeOrder()));
  for packet in &packet_feedback_vector {
    // Ignore packets without valid send or receive times.
    // (This should not happen in production since lost packets are filtered
    // out before passing the feedback vector to the throughput estimator.
    // However, explicitly handling this case makes the estimator more robust
    // and avoids a hard-to-detect bad state.)
    if (packet.receive_time.IsInfinite() ||
        packet.sent_packet.send_time.IsInfinite()) {
      continue;
    }

    // Insert the new packet.
    self.window.push_back(packet);
    self.window.back().sent_packet.prior_unacked_data =
        self.window.back().sent_packet.prior_unacked_data *
        self.settings.unacked_weight;
    // In most cases, receive timestamps should already be in order, but in the
    // rare case where feedback packets have been reordered, we do some swaps to
    // ensure that the window is sorted.
    let i: for = self.window.len() - 1;
         i > 0 && self.window[i].receive_time < self.window[i - 1].receive_time; i--) {
      std::swap(self.window[i], self.window[i - 1]);
    }
    const MaxReorderingTime: TimeDelta = TimeDelta::Seconds(1);
    const receive_delta: TimeDelta =
        (self.window.back().receive_time - packet.receive_time);
    if (receive_delta > MaxReorderingTime) {
      tracing::warn!(
          << "Severe packet re-ordering or timestamps offset changed: "
          << receive_delta;
      self.window.clear();
      self.latest_discarded_send_time = Timestamp::MinusInfinity();
    }
  }

  // Remove old packets.
  while (FirstPacketOutsideWindow()) {
    self.latest_discarded_send_time = std::cmp::max(
        self.latest_discarded_send_time, self.window.front().sent_packet.send_time);
    self.window.pop_front();
  }
}

bitrate: Option<DataRate>(&self /* RobustThroughputEstimator */) {
  if (self.window.is_empty() || self.window.len() < self.settings.required_packets)
    return None;

  TimeDelta largest_recv_gap(TimeDelta::Zero());
  TimeDelta second_largest_recv_gap(TimeDelta::Zero());
  let i: for = 1; i < self.window.len(); i++) {
    // Find receive time gaps.
    let gap: TimeDelta = self.window[i].receive_time - self.window[i - 1].receive_time;
    if (gap > largest_recv_gap) {
      second_largest_recv_gap = largest_recv_gap;
      largest_recv_gap = gap;
    } else if (gap > second_largest_recv_gap) {
      second_largest_recv_gap = gap;
    }
  }

  let first_send_time: Timestamp = Timestamp::PlusInfinity();
  let last_send_time: Timestamp = Timestamp::MinusInfinity();
  let first_recv_time: Timestamp = Timestamp::PlusInfinity();
  let last_recv_time: Timestamp = Timestamp::MinusInfinity();
  let recv_size: DataSize = DataSize::Bytes(0);
  let send_size: DataSize = DataSize::Bytes(0);
  let first_recv_size: DataSize = DataSize::Bytes(0);
  let last_send_size: DataSize = DataSize::Bytes(0);
  let num_sent_packets_in_window: usize = 0;
  for packet in &self.window {
    if (packet.receive_time < first_recv_time) {
      first_recv_time = packet.receive_time;
      first_recv_size =
          packet.sent_packet.size + packet.sent_packet.prior_unacked_data;
    }
    last_recv_time = std::cmp::max(last_recv_time, packet.receive_time);
    recv_size += packet.sent_packet.size;
    recv_size += packet.sent_packet.prior_unacked_data;

    if (packet.sent_packet.send_time < self.latest_discarded_send_time) {
      // If we have dropped packets from the window that were sent after
      // this packet, then this packet was reordered. Ignore it from
      // the send rate computation (since the send time may be very far
      // in the past, leading to underestimation of the send rate.)
      // However, ignoring packets creates a risk that we end up without
      // any packets left to compute a send rate.
      continue;
    }
    if (packet.sent_packet.send_time > last_send_time) {
      last_send_time = packet.sent_packet.send_time;
      last_send_size =
          packet.sent_packet.size + packet.sent_packet.prior_unacked_data;
    }
    first_send_time = std::cmp::min(first_send_time, packet.sent_packet.send_time);

    send_size += packet.sent_packet.size;
    send_size += packet.sent_packet.prior_unacked_data;
    num_sent_packets_in_window += 1;
  }

  // Suppose a packet of size S is sent every T milliseconds.
  // A window of N packets would contain N*S bytes, but the time difference
  // between the first and the last packet would only be (N-1)*T. Thus, we
  // need to remove the size of one packet to get the correct rate of S/T.
  // Which packet to remove (if the packets have varying sizes),
  // depends on the network model.
  // Suppose that 2 packets with sizes s1 and s2, are received at times t1
  // and t2, respectively. If the packets were transmitted back to back over
  // a bottleneck with rate capacity r, then we'd expect t2 = t1 + r * s2.
  // Thus, r = (t2-t1) / s2, so the size of the first packet doesn't affect
  // the difference between t1 and t2.
  // Analoguously, if the first packet is sent at time t1 and the sender
  // paces the packets at rate r, then the second packet can be sent at time
  // t2 = t1 + r * s1. Thus, the send rate estimate r = (t2-t1) / s1 doesn't
  // depend on the size of the last packet.
  recv_size -= first_recv_size;
  send_size -= last_send_size;

  // Remove the largest gap by replacing it by the second largest gap.
  // This is to ensure that spurious "delay spikes" (i.e. when the
  // network stops transmitting packets for a short period, followed
  // by a burst of delayed packets), don't cause the estimate to drop.
  // This could cause an overestimation, which we guard against by
  // never returning an estimate above the send rate.
  assert!(first_recv_time.IsFinite());
  assert!(last_recv_time.IsFinite());
  let recv_duration: TimeDelta = (last_recv_time - first_recv_time) -
                            largest_recv_gap + second_largest_recv_gap;
  recv_duration = std::cmp::max(recv_duration, TimeDelta::Millis(1));

  if (num_sent_packets_in_window < self.settings.required_packets) {
    // Too few send times to calculate a reliable send rate.
    return recv_size / recv_duration;
  }

  assert!(first_send_time.IsFinite());
  assert!(last_send_time.IsFinite());
  let send_duration: TimeDelta = last_send_time - first_send_time;
  send_duration = std::cmp::max(send_duration, TimeDelta::Millis(1));

  return std::cmp::min(send_size / send_duration, recv_size / recv_duration);
}

}  // namespace webrtc
