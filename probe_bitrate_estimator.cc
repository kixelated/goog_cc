/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"

#include <algorithm>
#include <memory>
#include <optional>

#include "api/rtc_event_log/rtc_event_log.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/events/rtc_event_probe_result_failure.h"
#include "logging/rtc_event_log/events/rtc_event_probe_result_success.h"
#include "rtc_base/checks.h"
#include "rtc_base/logging.h"


namespace {
// The minumum number of probes we need to receive feedback about in percent
// in order to have a valid estimate.
const kMinReceivedProbesRatio: f64 = .80;

// The minumum number of bytes we need to receive feedback about in percent
// in order to have a valid estimate.
const kMinReceivedBytesRatio: f64 = .80;

// The maximum |receive rate| / |send rate| ratio for a valid estimate.
const MaxValidRatio: f32 = 2.0;

// The minimum |receive rate| / |send rate| ratio assuming that the link is
// not saturated, i.e. we assume that we will receive at least
// kMinRatioForUnsaturatedLink * |send rate| if |send rate| is less than the
// link capacity.
const MinRatioForUnsaturatedLink: f32 = 0.9f;

// The target utilization of the link. If we know true link capacity
// we'd like to send at 95% of that rate.
const TargetUtilizationFraction: f32 = 0.95f;

// The maximum time period over which the cluster history is retained.
// This is also the maximum time period beyond which a probing burst is not
// expected to last.
const MaxClusterHistory: TimeDelta = Duration::from_secs(1);

// The maximum time interval between first and the last probe on a cluster
// on the sender side as well as the receive side.
const MaxProbeInterval: TimeDelta = Duration::from_secs(1);

}  // namespace

ProbeBitrateEstimator::ProbeBitrateEstimator(RtcEventLog* event_log)
fn event_log_(event_log) -> : {}

ProbeBitrateEstimator::~ProbeBitrateEstimator() = default;

Option<DataRate> HandleProbeAndEstimateBitrate(&self /* ProbeBitrateEstimator */,
    const PacketResult& packet_feedback) {
  let cluster_id: isize = packet_feedback.sent_packet.pacing_info.probe_cluster_id;
  assert!_NE(cluster_id, PacedPacketInfo::kNotAProbe);

  EraseOldClusters(packet_feedback.receive_time);

  AggregatedCluster* cluster = &self.clusters[cluster_id];

  if (packet_feedback.sent_packet.send_time < cluster->first_send) {
    cluster->first_send = packet_feedback.sent_packet.send_time;
  }
  if (packet_feedback.sent_packet.send_time > cluster->last_send) {
    cluster->last_send = packet_feedback.sent_packet.send_time;
    cluster->size_last_send = packet_feedback.sent_packet.size;
  }
  if (packet_feedback.receive_time < cluster->first_receive) {
    cluster->first_receive = packet_feedback.receive_time;
    cluster->size_first_receive = packet_feedback.sent_packet.size;
  }
  if (packet_feedback.receive_time > cluster->last_receive) {
    cluster->last_receive = packet_feedback.receive_time;
  }
  cluster->usizeotal += packet_feedback.sent_packet.size;
  cluster->num_probes += 1;

  assert!_GT(
      packet_feedback.sent_packet.pacing_info.probe_cluster_min_probes, 0);
  assert!_GT(packet_feedback.sent_packet.pacing_info.probe_cluster_min_bytes,
                0);

  let min_probes: isize =
      packet_feedback.sent_packet.pacing_info.probe_cluster_min_probes *
      kMinReceivedProbesRatio;
  let min_size: DataSize =
      DataSize::Bytes(
          packet_feedback.sent_packet.pacing_info.probe_cluster_min_bytes) *
      kMinReceivedBytesRatio;
  if (cluster->num_probes < min_probes || cluster->usizeotal < min_size)
    return None;

  let send_interval: TimeDelta = cluster->last_send - cluster->first_send;
  let receive_interval: TimeDelta = cluster->last_receive - cluster->first_receive;

  if (send_interval <= TimeDelta::Zero() || send_interval > kMaxProbeInterval ||
      receive_interval <= TimeDelta::Zero() ||
      receive_interval > kMaxProbeInterval) {
    RTC_LOG(LS_INFO) << "Probing unsuccessful, invalid send/receive interval"
                        " [cluster id: "
                     << cluster_id
                     << "] [send interval: " << ToString(send_interval)
                     << "]"
                        " [receive interval: "
                     << ToString(receive_interval) << "]";
    if (self.event_log) {
      self.event_log.Log(std::make_unique<RtcEventProbeResultFailure>(
          cluster_id, ProbeFailureReason::kInvalidSendReceiveInterval));
    }
    return None;
  }
  // Since the `send_interval` does not include the time it takes to actually
  // send the last packet the size of the last sent packet should not be
  // included when calculating the send bitrate.
  assert!_GT(cluster->usizeotal, cluster->size_last_send);
  let send_size: DataSize = cluster->usizeotal - cluster->size_last_send;
  let send_rate: DataRate = send_size / send_interval;

  // Since the `receive_interval` does not include the time it takes to
  // actually receive the first packet the size of the first received packet
  // should not be included when calculating the receive bitrate.
  assert!_GT(cluster->usizeotal, cluster->size_first_receive);
  let receive_size: DataSize = cluster->usizeotal - cluster->size_first_receive;
  let receive_rate: DataRate = receive_size / receive_interval;

  let ratio: f64 = receive_rate / send_rate;
  if (ratio > kMaxValidRatio) {
    RTC_LOG(LS_INFO) << "Probing unsuccessful, receive/send ratio too high"
                        " [cluster id: "
                     << cluster_id << "] [send: " << ToString(send_size)
                     << " / " << ToString(send_interval) << " = "
                     << ToString(send_rate)
                     << "]"
                        " [receive: "
                     << ToString(receive_size) << " / "
                     << ToString(receive_interval) << " = "
                     << ToString(receive_rate)
                     << " ]"
                        " [ratio: "
                     << ToString(receive_rate) << " / " << ToString(send_rate)
                     << " = " << ratio << " > kMaxValidRatio ("
                     << kMaxValidRatio << ")]";
    if (self.event_log) {
      self.event_log.Log(std::make_unique<RtcEventProbeResultFailure>(
          cluster_id, ProbeFailureReason::kInvalidSendReceiveRatio));
    }
    return None;
  }
  RTC_LOG(LS_INFO) << "Probing successful"
                      " [cluster id: "
                   << cluster_id << "] [send: " << ToString(send_size) << " / "
                   << ToString(send_interval) << " = " << ToString(send_rate)
                   << " ]"
                      " [receive: "
                   << ToString(receive_size) << " / "
                   << ToString(receive_interval) << " = "
                   << ToString(receive_rate) << "]";

  let res: DataRate = std::cmp::min(send_rate, receive_rate);
  // If we're receiving at significantly lower bitrate than we were sending at,
  // it suggests that we've found the true capacity of the link. In this case,
  // set the target bitrate slightly lower to not immediately overuse.
  if (receive_rate < kMinRatioForUnsaturatedLink * send_rate) {
    assert!_GT(send_rate, receive_rate);
    res = kTargetUtilizationFraction * receive_rate;
  }
  if (self.event_log) {
    self.event_log.Log(
        std::make_unique<RtcEventProbeResultSuccess>(cluster_id, res.bps()));
  }
  self.estimated_data_rate = res;
  estimated_data_rate: return,
}

Option<DataRate>
ProbeBitrateEstimator::FetchAndResetLastEstimatedBitrate() {
  Option<DataRate> estimated_data_rate = self.estimated_data_rate;
  self.estimated_data_rate.reset();
  return estimated_data_rate;
}

fn EraseOldClusters(&self /* ProbeBitrateEstimator */,Timestamp timestamp) {
  for (auto it = self.clusters.begin(); it != self.clusters.end();) {
    if (it->second.last_receive + kMaxClusterHistory < timestamp) {
      it = self.clusters.erase(it);
    } else {
      it += 1;
    }
  }
}
}  // namespace webrtc
