/*
 *  Copyright (c) 2012 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 *
 *  FEC and NACK added bitrate is handled outside class
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_SEND_SIDE_BANDWIDTH_ESTIMATION_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_SEND_SIDE_BANDWIDTH_ESTIMATION_H_

#include <stdint.h>

#include <deque>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include "api/field_trials_view.h"
#include "api/transport/bandwidth_usage.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/loss_based_bandwidth_estimation.h"
#include "modules/congestion_controller/goog_cc/loss_based_bwe_v2.h"
#include "rtc_base/experiments/field_trial_parser.h"



class RtcEventLog;

pub struct LinkCapacityTracker {
 public:
  LinkCapacityTracker() = default;
  ~LinkCapacityTracker() = default;
  // Call when a new delay-based estimate is available.
  void UpdateDelayBasedEstimate(Timestamp at_time,
                                DataRate delay_based_bitrate);
  fn OnStartingRate(DataRate start_rate) {
  todo!();
}
  void OnRateUpdate(Option<DataRate> acknowledged,
                    DataRate target,
                    Timestamp at_time);
  fn OnRttBackoff(DataRate backoff_rate, Timestamp at_time) {
  todo!();
}
  fn estimate(&self) -> DataRate;

 private:
  let capacity_estimate_bps_: f64 = 0.0;
  Timestamp self.last_link_capacity_update = Timestamp::MinusInfinity();
  DataRate self.last_delay_based_estimate = DataRate::PlusInfinity();
};

pub struct RttBasedBackoff {
 public:
  explicit RttBasedBackoff(const FieldTrialsView* key_value_config);
  ~RttBasedBackoff();
  fn UpdatePropagationRtt(Timestamp at_time, TimeDelta propagation_rtt) {
  todo!();
}
  fn IsRttAboveLimit(&self) -> bool;

  disabled: FieldTrialFlag,
  configured_limit: FieldTrialParameter<TimeDelta>,
  drop_fraction: FieldTrialParameter<f64>,
  drop_interval: FieldTrialParameter<TimeDelta>,
  bandwidth_floor: FieldTrialParameter<DataRate>,

 public:
  rtt_limit: TimeDelta,
  last_propagation_rtt_update: Timestamp,
  last_propagation_rtt: TimeDelta,
  last_packet_sent: Timestamp,

 private:
  fn CorrectedRtt(&self) -> TimeDelta;
};

pub struct SendSideBandwidthEstimation {
 public:
  SendSideBandwidthEstimation() = delete;
  SendSideBandwidthEstimation(const FieldTrialsView* key_value_config,
                              RtcEventLog* event_log);
  ~SendSideBandwidthEstimation();

  fn OnRouteChange(&mut self) { todo!(); }

  fn target_rate(&self) -> DataRate;
  fn loss_based_state(&self) -> LossBasedState;
  // Return whether the current rtt is higher than the rtt limited configured in
  // RttBasedBackoff.
  fn IsRttAboveLimit(&self) -> bool;
  uint8_t fraction_loss() { return self.last_fraction_loss; }
  TimeDelta round_trip_time() { return self.last_round_trip_time; }

  fn GetEstimatedLinkCapacity(&self) -> DataRate;
  // Call periodically to update estimate.
  fn UpdateEstimate(Timestamp at_time) {
  todo!();
}
  fn OnSentPacket(const SentPacket& sent_packet) {
  todo!();
}
  fn UpdatePropagationRtt(Timestamp at_time, TimeDelta propagation_rtt) {
  todo!();
}

  // Call when we receive a RTCP message with TMMBR or REMB.
  fn UpdateReceiverEstimate(Timestamp at_time, DataRate bandwidth) {
  todo!();
}

  // Call when a new delay-based estimate is available.
  fn UpdateDelayBasedEstimate(Timestamp at_time, DataRate bitrate) {
  todo!();
}

  // Call when we receive a RTCP message with a ReceiveBlock.
  void UpdatePacketsLost(i64 packets_lost,
                         i64 number_of_packets,
                         Timestamp at_time);

  // Call when we receive a RTCP message with a ReceiveBlock.
  fn UpdateRtt(TimeDelta rtt, Timestamp at_time) {
  todo!();
}

  void SetBitrates(Option<DataRate> send_bitrate,
                   DataRate min_bitrate,
                   DataRate max_bitrate,
                   Timestamp at_time);
  fn SetSendBitrate(DataRate bitrate, Timestamp at_time) {
  todo!();
}
  fn SetMinMaxBitrate(DataRate min_bitrate, DataRate max_bitrate) {
  todo!();
}
  fn GetMinBitrate(&self) -> isize;
  void SetAcknowledgedRate(Option<DataRate> acknowledged_rate,
                           Timestamp at_time);
  void UpdateLossBasedEstimator(const TransportPacketsFeedback& report,
                                BandwidthUsage delay_detector_state,
                                Option<DataRate> probe_bitrate,
                                bool in_alr);
  fn PaceAtLossBasedEstimate(&self) -> bool;

 private:
  friend class GoogCcStatePrinter;

  enum UmaState { NoUpdate, FirstDone, Done };

  fn IsInStartPhase(&self, at_time: Timestamp) -> bool;

  fn UpdateUmaStatsPacketsLost(Timestamp at_time, isize packets_lost) {
  todo!();
}

  // Updates history of min bitrates.
  // After this method returns self.min_bitrate_history.front().second contains the
  // min bitrate used during last BweIncreaseIntervalMs.
  fn UpdateMinHistory(Timestamp at_time) {
  todo!();
}

  // Gets the upper limit for the target bitrate. This is the minimum of the
  // delay based limit, the receiver limit and the loss based controller limit.
  fn GetUpperLimit(&self) -> DataRate;
  // Prints a warning if `bitrate` if sufficiently long time has past since last
  // warning.
  fn MaybeLogLowBitrateWarning(DataRate bitrate, Timestamp at_time) {
  todo!();
}
  // Stores an update to the event log if the loss rate has changed, the target
  // has changed, or sufficient time has passed since last stored event.
  fn MaybeLogLossBasedEvent(Timestamp at_time) {
  todo!();
}

  // Cap `bitrate` to [self.min_bitrate_configured, max_bitrate_configured_] and
  // set `current_bitrate_` to the capped value and updates the event log.
  fn UpdateTargetBitrate(DataRate bitrate, Timestamp at_time) {
  todo!();
}
  // Applies lower and upper bounds to the current target rate.
  // TODO(srte): This seems to be called even when limits haven't changed, that
  // should be cleaned up.
  fn ApplyTargetLimits(Timestamp at_time) {
  todo!();
}

  fn LossBasedBandwidthEstimatorV1Enabled(&self) -> bool;
  fn LossBasedBandwidthEstimatorV2Enabled(&self) -> bool;

  fn LossBasedBandwidthEstimatorV1ReadyForUse(&self) -> bool;
  fn LossBasedBandwidthEstimatorV2ReadyForUse(&self) -> bool;

  const FieldTrialsView* self.key_value_config;
  rtt_backoff: RttBasedBackoff,
  link_capacity: LinkCapacityTracker,

  VecDeque<std::pair<Timestamp, DataRate> > self.min_bitrate_history;

  // incoming filters
  lost_packets_since_last_loss_update: isize,
  expected_packets_since_last_loss_update: isize,

  acknowledged_rate: Option<DataRate>,
  current_target: DataRate,
  last_logged_target: DataRate,
  min_bitrate_configured: DataRate,
  max_bitrate_configured: DataRate,
  last_low_bitrate_log: Timestamp,

  has_decreased_since_last_fraction_loss: bool,
  last_loss_feedback: Timestamp,
  last_loss_packet_report: Timestamp,
  last_fraction_loss: uint8_t,
  last_logged_fraction_loss: uint8_t,
  last_round_trip_time: TimeDelta,

  // The max bitrate as set by the receiver in the call. This is typically
  // signalled using the REMB RTCP message and is used when we don't have any
  // send side delay based estimate.
  receiver_limit: DataRate,
  delay_based_limit: DataRate,
  time_last_decrease: Timestamp,
  first_report_time: Timestamp,
  initially_lost_packets: isize,
  bitrate_at_2_seconds: DataRate,
  uma_update_state: UmaState,
  uma_rtt_state: UmaState,
  rampup_uma_stats_updated: Vec<bool>,
  event_log: RtcEventLog*,
  last_rtc_event_log: Timestamp,
  low_loss_threshold: f32,
  high_loss_threshold: f32,
  bitrate_threshold: DataRate,
  loss_based_bandwidth_estimator_v1: LossBasedBandwidthEstimation,
  loss_based_bandwidth_estimator_v2: std::unique_ptr<LossBasedBweV2>,
  loss_based_state: LossBasedState,
  disable_receiver_limit_caps_only: FieldTrialFlag,
};
}  // namespace webrtc
#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_SEND_SIDE_BANDWIDTH_ESTIMATION_H_
