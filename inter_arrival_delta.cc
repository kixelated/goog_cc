/*
 *  Copyright (c) 2020 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/inter_arrival_delta.h"

#include <algorithm>
#include <cstddef>

#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "rtc_base/checks.h"
#include "rtc_base/logging.h"



static const BurstDeltaThreshold: TimeDelta = TimeDelta::Millis(5);
static const MaxBurstDuration: TimeDelta = TimeDelta::Millis(100);
constexpr TimeDelta InterArrivalDelta::kArrivalTimeOffsetThreshold;

InterArrivalDelta::InterArrivalDelta(TimeDelta send_time_group_length)
    : send_time_group_length_(send_time_group_length),
      current_timestamp_group_(),
      prev_timestamp_group_(),
      num_consecutive_reordered_packets_(0) {}

bool ComputeDeltas(&self /* InterArrivalDelta */,Timestamp send_time,
                                      Timestamp arrival_time,
                                      Timestamp system_time,
                                      usize packet_size,
                                      TimeDelta* send_time_delta,
                                      TimeDelta* arrival_time_delta,
                                      int* packet_size_delta) {
  let calculated_deltas: bool = false;
  if (self.current_timestamp_group.IsFirstPacket()) {
    // We don't have enough data to update the filter, so we store it until we
    // have two frames of data to process.
    self.current_timestamp_group.send_time = send_time;
    self.current_timestamp_group.first_send_time = send_time;
    self.current_timestamp_group.first_arrival = arrival_time;
  } else if (self.current_timestamp_group.first_send_time > send_time) {
    // Reordered packet.
    return false;
  } else if (NewTimestampGroup(arrival_time, send_time)) {
    // First packet of a later send burst, the previous packets sample is ready.
    if (self.prev_timestamp_group.complete_time.IsFinite()) {
      *send_time_delta =
          self.current_timestamp_group.send_time - self.prev_timestamp_group.send_time;
      *arrival_time_delta = self.current_timestamp_group.complete_time -
                            self.prev_timestamp_group.complete_time;

      let system_time_delta: TimeDelta = self.current_timestamp_group.last_system_time -
                                    self.prev_timestamp_group.last_system_time;

      if (*arrival_time_delta - system_time_delta >=
          kArrivalTimeOffsetThreshold) {
        RTC_LOG(LS_WARNING)
            << "The arrival time clock offset has changed (diff = "
            << arrival_time_delta->ms() - system_time_delta.ms()
            << " ms), resetting.";
        Reset();
        return false;
      }
      if (*arrival_time_delta < TimeDelta::Zero()) {
        // The group of packets has been reordered since receiving its local
        // arrival timestamp.
        self.num_consecutive_reordered_packets += 1;
        if (self.num_consecutive_reordered_packets >= kReorderedResetThreshold) {
          RTC_LOG(LS_WARNING)
              << "Packets between send burst arrived out of order, resetting:"
              << " arrival_time_delta_ms=" << arrival_time_delta->ms()
              << ", send_time_delta_ms=" << send_time_delta->ms();
          Reset();
        }
        return false;
      } else {
        self.num_consecutive_reordered_packets = 0;
      }
      *packet_size_delta = (self.current_timestamp_group.size) as isize -
                           (self.prev_timestamp_group.size) as isize;
      calculated_deltas = true;
    }
    self.prev_timestamp_group = self.current_timestamp_group;
    // The new timestamp is now the current frame.
    self.current_timestamp_group.first_send_time = send_time;
    self.current_timestamp_group.send_time = send_time;
    self.current_timestamp_group.first_arrival = arrival_time;
    self.current_timestamp_group.size = 0;
  } else {
    self.current_timestamp_group.send_time =
        std::cmp::max(self.current_timestamp_group.send_time, send_time);
  }
  // Accumulate the frame size.
  self.current_timestamp_group.size += packet_size;
  self.current_timestamp_group.complete_time = arrival_time;
  self.current_timestamp_group.last_system_time = system_time;

  return calculated_deltas;
}

// Assumes that `timestamp` is not reordered compared to
// `current_timestamp_group_`.
bool NewTimestampGroup(&self /* InterArrivalDelta */,Timestamp arrival_time,
                                          Timestamp send_time) {
  if (self.current_timestamp_group.IsFirstPacket()) {
    return false;
  } else if (BelongsToBurst(arrival_time, send_time)) {
    return false;
  } else {
    return send_time - self.current_timestamp_group.first_send_time >
           self.send_time_group_length;
  }
}

bool BelongsToBurst(&self /* InterArrivalDelta */,Timestamp arrival_time,
                                       Timestamp send_time) {
  assert!(self.current_timestamp_group.complete_time.IsFinite());
  let arrival_time_delta: TimeDelta =
      arrival_time - self.current_timestamp_group.complete_time;
  let send_time_delta: TimeDelta = send_time - self.current_timestamp_group.send_time;
  if (send_time_delta.IsZero())
    return true;
  let propagation_delta: TimeDelta = arrival_time_delta - send_time_delta;
  if (propagation_delta < TimeDelta::Zero() &&
      arrival_time_delta <= kBurstDeltaThreshold &&
      arrival_time - self.current_timestamp_group.first_arrival < kMaxBurstDuration)
    return true;
  return false;
}

fn Reset(&self /* InterArrivalDelta */) {
  self.num_consecutive_reordered_packets = 0;
  self.current_timestamp_group = SendTimeGroup();
  self.prev_timestamp_group = SendTimeGroup();
}
}  // namespace webrtc
