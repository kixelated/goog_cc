/*
 *  Copyright (c) 2020 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::api::units::{TimeDelta, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SendTimeGroup {
    size: usize,
    first_send_time: Timestamp,
    send_time: Timestamp,
    first_arrival: Timestamp,
    complete_time: Timestamp,
    last_system_time: Timestamp,
}

impl Default for SendTimeGroup {
    fn default() -> Self {
        Self {
            size: 0,
            first_send_time: Timestamp::MinusInfinity(),
            send_time: Timestamp::MinusInfinity(),
            first_arrival: Timestamp::MinusInfinity(),
            complete_time: Timestamp::MinusInfinity(),
            last_system_time: Timestamp::MinusInfinity(),
        }
    }
}
impl SendTimeGroup {
    pub fn IsFirstPacket(&self) -> bool {
        self.complete_time.IsInfinite()
    }
}

// Helper class to compute the inter-arrival time delta and the size delta
// between two send bursts. This code is branched from
// modules/remote_bitrate_estimator/inter_arrival.
pub struct InterArrivalDelta {
    send_time_group_length: TimeDelta,
    current_timestamp_group: SendTimeGroup,
    prev_timestamp_group: SendTimeGroup,
    num_consecutive_reordered_packets: isize,
}

impl InterArrivalDelta {
    const ReorderedResetThreshold: isize = 3;
    const ArrivalTimeOffsetThreshold: TimeDelta = TimeDelta::Seconds(3);
    const BurstDeltaThreshold: TimeDelta = TimeDelta::Millis(5);
    const MaxBurstDuration: TimeDelta = TimeDelta::Millis(100);

    pub fn new(send_time_group_length: TimeDelta) -> Self {
        Self {
            send_time_group_length,
            current_timestamp_group: SendTimeGroup::default(),
            prev_timestamp_group: SendTimeGroup::default(),
            num_consecutive_reordered_packets: 0,
        }
    }

    // This function returns true if a delta was computed, or false if the current
    // group is still incomplete or if only one group has been completed.
    // `send_time` is the send time.
    // `arrival_time` is the time at which the packet arrived.
    // `packet_size` is the size of the packet.
    // `timestamp_delta` (output) is the computed send time delta.
    // `arrival_time_delta` (output) is the computed arrival-time delta.
    // `packet_size_delta` (output) is the computed size delta.
    pub fn ComputeDeltas(
        &mut self,
        send_time: Timestamp,
        arrival_time: Timestamp,
        system_time: Timestamp,
        packet_size: usize,
        send_time_delta: &mut TimeDelta,
        arrival_time_delta: &mut TimeDelta,
        packet_size_delta: &mut isize,
    ) -> bool {
        let mut calculated_deltas: bool = false;
        if self.current_timestamp_group.IsFirstPacket() {
            // We don't have enough data to update the filter, so we store it until we
            // have two frames of data to process.
            self.current_timestamp_group.send_time = send_time;
            self.current_timestamp_group.first_send_time = send_time;
            self.current_timestamp_group.first_arrival = arrival_time;
        } else if self.current_timestamp_group.first_send_time > send_time {
            // Reordered packet.
            return false;
        } else if self.NewTimestampGroup(arrival_time, send_time) {
            // First packet of a later send burst, the previous packets sample is ready.
            if self.prev_timestamp_group.complete_time.IsFinite() {
                *send_time_delta =
                    self.current_timestamp_group.send_time - self.prev_timestamp_group.send_time;
                *arrival_time_delta = self.current_timestamp_group.complete_time
                    - self.prev_timestamp_group.complete_time;

                let system_time_delta: TimeDelta = self.current_timestamp_group.last_system_time
                    - self.prev_timestamp_group.last_system_time;

                if *arrival_time_delta - system_time_delta >= Self::ArrivalTimeOffsetThreshold {
                    tracing::warn!(
                        "The arrival time clock offset has changed (diff = {} ms), resetting.",
                        arrival_time_delta.ms() - system_time_delta.ms()
                    );
                    self.Reset();
                    return false;
                }
                if *arrival_time_delta < TimeDelta::Zero() {
                    // The group of packets has been reordered since receiving its local
                    // arrival timestamp.
                    self.num_consecutive_reordered_packets += 1;
                    if self.num_consecutive_reordered_packets >= Self::ReorderedResetThreshold {
                        tracing::warn!("Packets between send burst arrived out of order, resetting: arrival_time_delta_ms={}, send_time_delta_ms={}", arrival_time_delta.ms(), send_time_delta.ms());
                        self.Reset();
                    }
                    return false;
                } else {
                    self.num_consecutive_reordered_packets = 0;
                }
                *packet_size_delta = (self.current_timestamp_group.size) as isize
                    - (self.prev_timestamp_group.size) as isize;
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

        calculated_deltas
    }

    // Returns true if the last packet was the end of the current batch and the
    // packet with `send_time` is the first of a new batch.
    fn NewTimestampGroup(&self, arrival_time: Timestamp, send_time: Timestamp) -> bool {
        if self.current_timestamp_group.IsFirstPacket() || self.BelongsToBurst(arrival_time, send_time) {
            false
        } else {
            send_time - self.current_timestamp_group.first_send_time
                > self.send_time_group_length
        }
    }

    fn BelongsToBurst(&self, arrival_time: Timestamp, send_time: Timestamp) -> bool {
        assert!(self.current_timestamp_group.complete_time.IsFinite());
        let arrival_time_delta: TimeDelta =
            arrival_time - self.current_timestamp_group.complete_time;
        let send_time_delta: TimeDelta = send_time - self.current_timestamp_group.send_time;
        if send_time_delta.IsZero() {
            return true;
        }
        let propagation_delta: TimeDelta = arrival_time_delta - send_time_delta;
        if propagation_delta < TimeDelta::Zero()
            && arrival_time_delta <= Self::BurstDeltaThreshold
            && arrival_time - self.current_timestamp_group.first_arrival < Self::MaxBurstDuration
        {
            return true;
        }
        false
    }

    fn Reset(&mut self) {
        self.num_consecutive_reordered_packets = 0;
        self.current_timestamp_group = SendTimeGroup::default();
        self.prev_timestamp_group = SendTimeGroup::default();
    }
}
