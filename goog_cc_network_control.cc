/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/goog_cc_network_control.h"

#include <stdio.h>

#include <algorithm>
#include <cstdint>
#include <memory>
#include <numeric>
#include <optional>
#include <utility>
#include <vector>

#include "absl/strings/string_view.h"
#include "api/environment/environment.h"
#include "api/field_trials_view.h"
#include "api/transport/bandwidth_usage.h"
#include "api/transport/network_control.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/events/rtc_event_remote_estimate.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"
#include "modules/congestion_controller/goog_cc/alr_detector.h"
#include "modules/congestion_controller/goog_cc/congestion_window_pushback_controller.h"
#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"
#include "modules/congestion_controller/goog_cc/loss_based_bwe_v2.h"
#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"
#include "modules/congestion_controller/goog_cc/probe_controller.h"
#include "modules/congestion_controller/goog_cc/send_side_bandwidth_estimation.h"
#include "modules/remote_bitrate_estimator/include/bwe_defines.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/field_trial_parser.h"
#include "rtc_base/experiments/rate_control_settings.h"
#include "rtc_base/logging.h"



namespace {
// From RTCPSender video report interval.
constexpr TimeDelta kLossUpdateInterval = TimeDelta::Millis(1000);

// Pacing-rate relative to our target send rate.
// Multiplicative factor that is applied to the target bitrate to calculate
// the number of bytes that can be transmitted per interval.
// Increasing this factor will result in lower delays in cases of bitrate
// overshoots from the encoder.
constexpr float kDefaultPaceMultiplier = 2.5f;

// If the probe result is far below the current throughput estimate
// it's unlikely that the probe is accurate, so we don't want to drop too far.
// However, if we actually are overusing, we want to drop to something slightly
// below the current throughput estimate to drain the network queues.
const kProbeDropThroughputFraction: f64 = 0.85;

BandwidthLimitedCause GetBandwidthLimitedCause(LossBasedState loss_based_state,
                                               bool is_rtt_above_limit,
                                               BandwidthUsage bandwidth_usage) {
  if (bandwidth_usage == BandwidthUsage::kBwOverusing ||
      bandwidth_usage == BandwidthUsage::kBwUnderusing) {
    return BandwidthLimitedCause::kDelayBasedLimitedDelayIncreased;
  } else if (is_rtt_above_limit) {
    return BandwidthLimitedCause::kRttBasedBackOffHighRtt;
  }
  switch (loss_based_state) {
    case LossBasedState::kDecreasing:
      // Probes may not be sent in this state.
      return BandwidthLimitedCause::kLossLimitedBwe;
    case webrtc::LossBasedState::kIncreaseUsingPadding:
      // Probes may not be sent in this state.
      return BandwidthLimitedCause::kLossLimitedBwe;
    case LossBasedState::kIncreasing:
      // Probes may be sent in this state.
      return BandwidthLimitedCause::kLossLimitedBweIncreasing;
    case LossBasedState::kDelayBasedEstimate:
      return BandwidthLimitedCause::kDelayBasedLimited;
  }
}

}  // namespace

GoogCcNetworkController::GoogCcNetworkController(NetworkControllerConfig config,
                                                 GoogCcConfig goog_cc_config)
    : env_(config.env),
      packet_feedback_only_(goog_cc_config.feedback_only),
      safe_reset_on_route_change_("Enabled"),
      safe_reset_acknowledged_rate_("ack"),
      use_min_allocatable_as_lower_bound_(
          !self.env.field_trials().IsDisabled("WebRTC-Bwe-MinAllocAsLowerBound")),
      ignore_probes_lower_than_network_estimate_(
          !self.env.field_trials().IsDisabled(
              "WebRTC-Bwe-IgnoreProbesLowerThanNetworkStateEstimate")),
      limit_probes_lower_than_throughput_estimate_(
          !self.env.field_trials().IsDisabled(
              "WebRTC-Bwe-LimitProbesLowerThanThroughputEstimate")),
      rate_control_settings_(self.env.field_trials()),
      pace_at_max_of_bwe_and_lower_link_capacity_(self.env.field_trials().IsEnabled(
          "WebRTC-Bwe-PaceAtMaxOfBweAndLowerLinkCapacity")),
      limit_pacingfactor_by_upper_link_capacity_estimate_(
          self.env.field_trials().IsEnabled(
              "WebRTC-Bwe-LimitPacingFactorByUpperLinkCapacityEstimate")),
      probe_controller_(
          new ProbeController(&self.env.field_trials(), &self.env.event_log())),
      congestion_window_pushback_controller_(
          self.rate_control_settings.UseCongestionWindowPushback()
              ? std::make_unique<CongestionWindowPushbackController>(
                    self.env.field_trials())
              : nullptr),
      bandwidth_estimation_(
          std::make_unique<SendSideBandwidthEstimation>(&self.env.field_trials(),
                                                        &self.env.event_log())),
      alr_detector_(std::make_unique<AlrDetector>(&self.env.field_trials(),
                                                  &self.env.event_log())),
      probe_bitrate_estimator_(new ProbeBitrateEstimator(&self.env.event_log())),
      network_estimator_(std::move(goog_cc_config.network_state_estimator)),
      network_state_predictor_(
          std::move(goog_cc_config.network_state_predictor)),
      delay_based_bwe_(new DelayBasedBwe(&self.env.field_trials(),
                                         &self.env.event_log(),
                                         self.network_state_predictor.get())),
      acknowledged_bitrate_estimator_(
          AcknowledgedBitrateEstimatorInterface::Create(&self.env.field_trials())),
      initial_config_(config),
      last_loss_based_target_rate_(*config.constraints.starting_rate),
      last_pushback_target_rate_(self.last_loss_based_target_rate),
      last_stable_target_rate_(self.last_loss_based_target_rate),
      last_loss_base_state_(LossBasedState::kDelayBasedEstimate),
      pacing_factor_(config.stream_based_config.pacing_factor.unwrap_or(
          kDefaultPaceMultiplier)),
      min_total_allocated_bitrate_(
          config.stream_based_config.min_total_allocated_bitrate.unwrap_or(
              DataRate::Zero())),
      max_padding_rate_(config.stream_based_config.max_padding_rate.unwrap_or(
          DataRate::Zero())) {
  assert!(config.constraints.at_time.IsFinite());
  ParseFieldTrial(
      {&self.safe_reset_on_route_change, &safe_reset_acknowledged_rate_},
      self.env.field_trials().Lookup("WebRTC-Bwe-SafeResetOnRouteChange"));
  if (self.delay_based_bwe)
    self.delay_based_bwe.SetMinBitrate(kCongestionControllerMinBitrate);
}

GoogCcNetworkController::~GoogCcNetworkController() {}

NetworkControlUpdate OnNetworkAvailability(&self /* GoogCcNetworkController */,
    NetworkAvailability msg) {
  NetworkControlUpdate update;
  update.probe_cluster_configs = self.probe_controller.OnNetworkAvailability(msg);
  return update;
}

NetworkControlUpdate OnNetworkRouteChange(&self /* GoogCcNetworkController */,
    NetworkRouteChange msg) {
  if (self.safe_reset_on_route_change) {
    Option<DataRate> estimated_bitrate;
    if (self.safe_reset_acknowledged_rate) {
      estimated_bitrate = self.acknowledged_bitrate_estimator.bitrate();
      if (!estimated_bitrate)
        estimated_bitrate = self.acknowledged_bitrate_estimator.PeekRate();
    } else {
      estimated_bitrate = self.bandwidth_estimation.target_rate();
    }
    if (estimated_bitrate) {
      if (msg.constraints.starting_rate) {
        msg.constraints.starting_rate =
            std::cmp::min(*msg.constraints.starting_rate, *estimated_bitrate);
      } else {
        msg.constraints.starting_rate = estimated_bitrate;
      }
    }
  }

  self.acknowledged_bitrate_estimator =
      AcknowledgedBitrateEstimatorInterface::Create(&self.env.field_trials());
  self.probe_bitrate_estimator.reset(new ProbeBitrateEstimator(&self.env.event_log()));
  if (self.network_estimator)
    self.network_estimator.OnRouteChange(msg);
  self.delay_based_bwe.reset(new DelayBasedBwe(
      &self.env.field_trials(), &self.env.event_log(), self.network_state_predictor.get()));
  self.bandwidth_estimation.OnRouteChange();
  self.probe_controller.Reset(msg.at_time);
  NetworkControlUpdate update;
  update.probe_cluster_configs = ResetConstraints(msg.constraints);
  MaybeTriggerOnNetworkChanged(&update, msg.at_time);
  return update;
}

NetworkControlUpdate OnProcessInterval(&self /* GoogCcNetworkController */,
    ProcessInterval msg) {
  NetworkControlUpdate update;
  if (self.initial_config) {
    update.probe_cluster_configs =
        ResetConstraints(self.initial_config.constraints);
    update.pacer_config = GetPacingRates(msg.at_time);

    if (self.initial_config.stream_based_config.requests_alr_probing) {
      self.probe_controller.EnablePeriodicAlrProbing(
          *self.initial_config.stream_based_config.requests_alr_probing);
    }
    if (self.initial_config.stream_based_config.enable_repeated_initial_probing) {
      self.probe_controller.EnableRepeatedInitialProbing(
          *self.initial_config.stream_based_config
               .enable_repeated_initial_probing);
    }
    Option<DataRate> total_bitrate =
        self.initial_config.stream_based_config.max_total_allocated_bitrate;
    if (total_bitrate) {
let probes = self.probe_controller.OnMaxTotalAllocatedBitrate(
          *total_bitrate, msg.at_time);
      update.probe_cluster_configs.insert(update.probe_cluster_configs.end(),
                                          probes.begin(), probes.end());
    }
    self.initial_config.reset();
  }
  if (self.congestion_window_pushback_controller && msg.pacer_queue) {
    self.congestion_window_pushback_controller.UpdatePacingQueue(
        msg.pacer_queue->bytes());
  }
  self.bandwidth_estimation.UpdateEstimate(msg.at_time);
  Option<i64> start_time_ms =
      self.alr_detector.GetApplicationLimitedRegionStartTime();
  self.probe_controller.SetAlrStartTimeMs(start_time_ms);

let probes = self.probe_controller.Process(msg.at_time);
  update.probe_cluster_configs.insert(update.probe_cluster_configs.end(),
                                      probes.begin(), probes.end());

  if (self.rate_control_settings.UseCongestionWindow() &&
      !self.feedback_max_rtts.empty()) {
    UpdateCongestionWindowlen();
  }
  if (self.congestion_window_pushback_controller && self.current_data_window) {
    self.congestion_window_pushback_controller.SetDataWindow(
        *self.current_data_window);
  } else {
    update.congestion_window = self.current_data_window;
  }
  MaybeTriggerOnNetworkChanged(&update, msg.at_time);
  return update;
}

NetworkControlUpdate OnRemoteBitrateReport(&self /* GoogCcNetworkController */,
    RemoteBitrateReport msg) {
  if (self.packet_feedback_only) {
    RTC_LOG(LS_ERROR) << "Received REMB for packet feedback only GoogCC";
    return NetworkControlUpdate();
  }
  self.bandwidth_estimation.UpdateReceiverEstimate(msg.receive_time,
                                                msg.bandwidth);
  return NetworkControlUpdate();
}

NetworkControlUpdate OnRoundTripTimeUpdate(&self /* GoogCcNetworkController */,
    RoundTripTimeUpdate msg) {
  if (packet_feedback_only_ || msg.smoothed)
    return NetworkControlUpdate();
  assert!(!msg.round_trip_time.IsZero());
  if (self.delay_based_bwe)
    self.delay_based_bwe.OnRttUpdate(msg.round_trip_time);
  self.bandwidth_estimation.UpdateRtt(msg.round_trip_time, msg.receive_time);
  return NetworkControlUpdate();
}

NetworkControlUpdate OnSentPacket(&self /* GoogCcNetworkController */,
    SentPacket sent_packet) {
  self.alr_detector.OnBytesSent(sent_packet.size.bytes(),
                             sent_packet.send_time.ms());
  self.acknowledged_bitrate_estimator.SetAlr(
      self.alr_detector.GetApplicationLimitedRegionStartTime().is_some());

  if (!self.first_packet_sent) {
    self.first_packet_sent = true;
    // Initialize feedback time to send time to allow estimation of RTT until
    // first feedback is received.
    self.bandwidth_estimation.UpdatePropagationRtt(sent_packet.send_time,
                                                TimeDelta::Zero());
  }
  self.bandwidth_estimation.OnSentPacket(sent_packet);

  if (self.congestion_window_pushback_controller) {
    self.congestion_window_pushback_controller.UpdateOutstandingData(
        sent_packet.data_in_flight.bytes());
    NetworkControlUpdate update;
    MaybeTriggerOnNetworkChanged(&update, sent_packet.send_time);
    return update;
  } else {
    return NetworkControlUpdate();
  }
}

NetworkControlUpdate OnReceivedPacket(&self /* GoogCcNetworkController */,
    ReceivedPacket /* received_packet */) {
  return NetworkControlUpdate();
}

NetworkControlUpdate OnStreamsConfig(&self /* GoogCcNetworkController */,
    StreamsConfig msg) {
  NetworkControlUpdate update;
  if (msg.requests_alr_probing) {
    self.probe_controller.EnablePeriodicAlrProbing(*msg.requests_alr_probing);
  }
  if (msg.max_total_allocated_bitrate) {
    update.probe_cluster_configs =
        self.probe_controller.OnMaxTotalAllocatedBitrate(
            *msg.max_total_allocated_bitrate, msg.at_time);
  }

  bool pacing_changed = false;
  if (msg.pacing_factor && *msg.pacing_factor != self.pacing_factor) {
    self.pacing_factor = *msg.pacing_factor;
    pacing_changed = true;
  }
  if (msg.min_total_allocated_bitrate &&
      *msg.min_total_allocated_bitrate != self.min_total_allocated_bitrate) {
    self.min_total_allocated_bitrate = *msg.min_total_allocated_bitrate;
    pacing_changed = true;

    if (self.use_min_allocatable_as_lower_bound) {
      ClampConstraints();
      self.delay_based_bwe.SetMinBitrate(self.min_data_rate);
      self.bandwidth_estimation.SetMinMaxBitrate(self.min_data_rate, self.max_data_rate);
    }
  }
  if (msg.max_padding_rate && *msg.max_padding_rate != self.max_padding_rate) {
    self.max_padding_rate = *msg.max_padding_rate;
    pacing_changed = true;
  }

  if (pacing_changed)
    update.pacer_config = GetPacingRates(msg.at_time);
  return update;
}

NetworkControlUpdate OnTargetRateConstraints(&self /* GoogCcNetworkController */,
    TargetRateConstraints constraints) {
  NetworkControlUpdate update;
  update.probe_cluster_configs = ResetConstraints(constraints);
  MaybeTriggerOnNetworkChanged(&update, constraints.at_time);
  return update;
}

fn ClampConstraints(&self /* GoogCcNetworkController */) {
  // TODO(holmer): We should make sure the default bitrates are set to 10 kbps,
  // and that we don't try to set the min bitrate to 0 from any applications.
  // The congestion controller should allow a min bitrate of 0.
  self.min_data_rate = std::cmp::max(self.min_target_rate, kCongestionControllerMinBitrate);
  if (self.use_min_allocatable_as_lower_bound) {
    self.min_data_rate = std::cmp::max(self.min_data_rate, self.min_total_allocated_bitrate);
  }
  if (self.max_data_rate < self.min_data_rate) {
    tracing::warn!( "max bitrate smaller than min bitrate");
    self.max_data_rate = self.min_data_rate;
  }
  if (self.starting_rate && self.starting_rate < self.min_data_rate) {
    tracing::warn!( "start bitrate smaller than min bitrate");
    self.starting_rate = self.min_data_rate;
  }
}

Vec<ProbeClusterConfig> ResetConstraints(&self /* GoogCcNetworkController */,
    TargetRateConstraints new_constraints) {
  self.min_target_rate = new_constraints.min_data_rate.unwrap_or(DataRate::Zero());
  self.max_data_rate =
      new_constraints.max_data_rate.unwrap_or(DataRate::PlusInfinity());
  self.starting_rate = new_constraints.starting_rate;
  ClampConstraints();

  self.bandwidth_estimation.SetBitrates(self.starting_rate, self.min_data_rate,
                                     self.max_data_rate, new_constraints.at_time);

  if (self.starting_rate)
    self.delay_based_bwe.SetStartBitrate(*self.starting_rate);
  self.delay_based_bwe.SetMinBitrate(self.min_data_rate);

  return self.probe_controller.SetBitrates(
      self.min_data_rate, self.starting_rate.unwrap_or(DataRate::Zero()), self.max_data_rate,
      new_constraints.at_time);
}

NetworkControlUpdate OnTransportLossReport(&self /* GoogCcNetworkController */,
    TransportLossReport msg) {
  if (self.packet_feedback_only)
    return NetworkControlUpdate();
  i64 total_packets_delta =
      msg.packets_received_delta + msg.packets_lost_delta;
  self.bandwidth_estimation.UpdatePacketsLost(
      msg.packets_lost_delta, total_packets_delta, msg.receive_time);
  return NetworkControlUpdate();
}

fn UpdateCongestionWindowSize(&self /* GoogCcNetworkController */) {
  TimeDelta min_feedback_max_rtt = TimeDelta::Millis(
      *std::cmp::min_element(self.feedback_max_rtts.begin(), self.feedback_max_rtts.end()));

  const DataSize kMinCwnd = DataSize::Bytes(2 * 1500);
  TimeDelta time_window =
      min_feedback_max_rtt +
      TimeDelta::Millis(
          self.rate_control_settings.GetCongestionWindowAdditionalTimeMs());

  DataSize data_window = self.last_loss_based_target_rate * time_window;
  if (self.current_data_window) {
    data_window =
        std::cmp::max(kMinCwnd, (data_window + self.current_data_window.value()) / 2);
  } else {
    data_window = std::cmp::max(kMinCwnd, data_window);
  }
  self.current_data_window = data_window;
}

NetworkControlUpdate OnTransportPacketsFeedback(&self /* GoogCcNetworkController */,
    TransportPacketsFeedback report) {
  if (report.packet_feedbacks.empty()) {
    // TODO(bugs.webrtc.org/10125): Design a better mechanism to safe-guard
    // against building very large network queues.
    return NetworkControlUpdate();
  }

  if (self.congestion_window_pushback_controller) {
    self.congestion_window_pushback_controller.UpdateOutstandingData(
        report.data_in_flight.bytes());
  }
  TimeDelta max_feedback_rtt = TimeDelta::MinusInfinity();
  TimeDelta min_propagation_rtt = TimeDelta::PlusInfinity();
  Timestamp max_recv_time = Timestamp::MinusInfinity();

  Vec<PacketResult> feedbacks = report.ReceivedWithSendInfo();
  for (const auto& feedback : feedbacks)
    max_recv_time = std::cmp::max(max_recv_time, feedback.receive_time);

  for feedback in &feedbacks {
    TimeDelta feedback_rtt =
        report.feedback_time - feedback.sent_packet.send_time;
    TimeDelta min_pending_time = max_recv_time - feedback.receive_time;
    TimeDelta propagation_rtt = feedback_rtt - min_pending_time;
    max_feedback_rtt = std::cmp::max(max_feedback_rtt, feedback_rtt);
    min_propagation_rtt = std::cmp::min(min_propagation_rtt, propagation_rtt);
  }

  if (max_feedback_rtt.IsFinite()) {
    self.feedback_max_rtts.push_back(max_feedback_rtt.ms());
    const usize kMaxFeedbackRttWindow = 32;
    if (self.feedback_max_rtts.len() > kMaxFeedbackRttWindow)
      self.feedback_max_rtts.pop_front();
    // TODO(srte): Use time since last unacknowledged packet.
    self.bandwidth_estimation.UpdatePropagationRtt(report.feedback_time,
                                                min_propagation_rtt);
  }
  if (self.packet_feedback_only) {
    if (!self.feedback_max_rtts.empty()) {
      i64 sum_rtt_ms =
          std::accumulate(self.feedback_max_rtts.begin(), self.feedback_max_rtts.end(),
                          static_cast<i64>(0));
      i64 mean_rtt_ms = sum_rtt_ms / self.feedback_max_rtts.len();
      if (self.delay_based_bwe)
        self.delay_based_bwe.OnRttUpdate(TimeDelta::Millis(mean_rtt_ms));
    }

    TimeDelta feedback_min_rtt = TimeDelta::PlusInfinity();
    for packet_feedback in &feedbacks {
      TimeDelta pending_time = max_recv_time - packet_feedback.receive_time;
      TimeDelta rtt = report.feedback_time -
                      packet_feedback.sent_packet.send_time - pending_time;
      // Value used for predicting NACK round trip time in FEC controller.
      feedback_min_rtt = std::cmp::min(rtt, feedback_min_rtt);
    }
    if (feedback_min_rtt.IsFinite()) {
      self.bandwidth_estimation.UpdateRtt(feedback_min_rtt, report.feedback_time);
    }

    self.expected_packets_since_last_loss_update +=
        report.PacketsWithFeedback().len();
    for (const auto& packet_feedback : report.PacketsWithFeedback()) {
      if (!packet_feedback.IsReceived())
        self.lost_packets_since_last_loss_update += 1;
    }
    if (report.feedback_time > self.next_loss_update) {
      self.next_loss_update = report.feedback_time + kLossUpdateInterval;
      self.bandwidth_estimation.UpdatePacketsLost(
          self.lost_packets_since_last_loss_update,
          self.expected_packets_since_last_loss_update, report.feedback_time);
      self.expected_packets_since_last_loss_update = 0;
      self.lost_packets_since_last_loss_update = 0;
    }
  }
  Option<i64> alr_start_time =
      self.alr_detector.GetApplicationLimitedRegionStartTime();

  if (self.previously_in_alr && !alr_start_time.is_some()) {
    i64 now_ms = report.feedback_time.ms();
    self.acknowledged_bitrate_estimator.SetAlrEndedTime(report.feedback_time);
    self.probe_controller.SetAlrEndedTimeMs(now_ms);
  }
  self.previously_in_alr = alr_start_time.is_some();
  self.acknowledged_bitrate_estimator.IncomingPacketFeedbackVector(
      report.SortedByReceiveTime());
let acknowledged_bitrate = self.acknowledged_bitrate_estimator.bitrate();
  self.bandwidth_estimation.SetAcknowledgedRate(acknowledged_bitrate,
                                             report.feedback_time);
  for (const auto& feedback : report.SortedByReceiveTime()) {
    if (feedback.sent_packet.pacing_info.probe_cluster_id !=
        PacedPacketInfo::kNotAProbe) {
      self.probe_bitrate_estimator.HandleProbeAndEstimateBitrate(feedback);
    }
  }

  if (self.network_estimator) {
    self.network_estimator.OnTransportPacketsFeedback(report);
    SetNetworkStateEstimate(self.network_estimator.GetCurrentEstimate());
  }
  Option<DataRate> probe_bitrate =
      self.probe_bitrate_estimator.FetchAndResetLastEstimatedBitrate();
  if (self.ignore_probes_lower_than_network_estimate && probe_bitrate &&
      self.estimate && *probe_bitrate < self.delay_based_bwe.last_estimate() &&
      *probe_bitrate < self.estimate.link_capacity_lower) {
    probe_bitrate.reset();
  }
  if (self.limit_probes_lower_than_throughput_estimate && probe_bitrate &&
      acknowledged_bitrate) {
    // Limit the backoff to something slightly below the acknowledged
    // bitrate. ("Slightly below" because we want to drain the queues
    // if we are actually overusing.)
    // The acknowledged bitrate shouldn't normally be higher than the delay
    // based estimate, but it could happen e.g. due to packet bursts or
    // encoder overshoot. We use std::cmp::min to ensure that a probe result
    // below the current BWE never causes an increase.
    DataRate limit =
        std::cmp::min(self.delay_based_bwe.last_estimate(),
                 *acknowledged_bitrate * kProbeDropThroughputFraction);
    probe_bitrate = std::cmp::max(*probe_bitrate, limit);
  }

  NetworkControlUpdate update;
  bool recovered_from_overuse = false;

  DelayBasedBwe::Result result;
  result = self.delay_based_bwe.IncomingPacketFeedbackVector(
      report, acknowledged_bitrate, probe_bitrate, self.estimate,
      alr_start_time.is_some());

  if (result.updated) {
    if (result.probe) {
      self.bandwidth_estimation.SetSendBitrate(result.target_bitrate,
                                            report.feedback_time);
    }
    // Since SetSendBitrate now resets the delay-based estimate, we have to
    // call UpdateDelayBasedEstimate after SetSendBitrate.
    self.bandwidth_estimation.UpdateDelayBasedEstimate(report.feedback_time,
                                                    result.target_bitrate);
  }
  self.bandwidth_estimation.UpdateLossBasedEstimator(
      report, result.delay_detector_state, probe_bitrate,
      alr_start_time.is_some());
  if (result.updated) {
    // Update the estimate in the ProbeController, in case we want to probe.
    MaybeTriggerOnNetworkChanged(&update, report.feedback_time);
  }

  recovered_from_overuse = result.recovered_from_overuse;

  if (recovered_from_overuse) {
    self.probe_controller.SetAlrStartTimeMs(alr_start_time);
let probes = self.probe_controller.RequestProbe(report.feedback_time);
    update.probe_cluster_configs.insert(update.probe_cluster_configs.end(),
                                        probes.begin(), probes.end());
  }

  // No valid RTT could be because send-side BWE isn't used, in which case
  // we don't try to limit the outstanding packets.
  if (self.rate_control_settings.UseCongestionWindow() &&
      max_feedback_rtt.IsFinite()) {
    UpdateCongestionWindowlen();
  }
  if (self.congestion_window_pushback_controller && self.current_data_window) {
    self.congestion_window_pushback_controller.SetDataWindow(
        *self.current_data_window);
  } else {
    update.congestion_window = self.current_data_window;
  }

  return update;
}

NetworkControlUpdate OnNetworkStateEstimate(&self /* GoogCcNetworkController */,
    NetworkStateEstimate msg) {
  if (!self.network_estimator) {
    SetNetworkStateEstimate(msg);
  }
  return NetworkControlUpdate();
}

fn SetNetworkStateEstimate(&self /* GoogCcNetworkController */,
    Option<NetworkStateEstimate> estimate) {
let prev_estimate = self.estimate;
  self.estimate = estimate;
  if (self.estimate && (!prev_estimate ||
                    self.estimate.update_time != prev_estimate->update_time)) {
    self.env.event_log().Log(std::make_unique<RtcEventRemoteEstimate>(
        self.estimate.link_capacity_lower, self.estimate.link_capacity_upper));
    self.probe_controller.SetNetworkStateEstimate(*self.estimate);
  }
}

NetworkControlUpdate GetNetworkState(&self /* GoogCcNetworkController */,
    Timestamp at_time) {
  NetworkControlUpdate update;
  update.target_rate = TargetTransferRate();
  update.target_rate->network_estimate.at_time = at_time;
  update.target_rate->network_estimate.loss_rate_ratio =
      self.last_estimated_fraction_loss.unwrap_or(0) / 255.0;
  update.target_rate->network_estimate.round_trip_time =
      self.last_estimated_round_trip_time;
  update.target_rate->network_estimate.bwe_period =
      self.delay_based_bwe.GetExpectedBwePeriod();

  update.target_rate->at_time = at_time;
  update.target_rate->target_rate = self.last_pushback_target_rate;
  update.target_rate->stable_target_rate =
      self.bandwidth_estimation.GetEstimatedLinkCapacity();
  update.pacer_config = GetPacingRates(at_time);
  update.congestion_window = self.current_data_window;
  return update;
}

fn MaybeTriggerOnNetworkChanged(&self /* GoogCcNetworkController */,
    NetworkControlUpdate* update,
    Timestamp at_time) {
  uint8_t fraction_loss = self.bandwidth_estimation.fraction_loss();
  TimeDelta round_trip_time = self.bandwidth_estimation.round_trip_time();
  DataRate loss_based_target_rate = self.bandwidth_estimation.target_rate();
  LossBasedState loss_based_state = self.bandwidth_estimation.loss_based_state();
  DataRate pushback_target_rate = loss_based_target_rate;

  let cwnd_reduce_ratio: f64 = 0.0;
  if (self.congestion_window_pushback_controller) {
    i64 pushback_rate =
        self.congestion_window_pushback_controller.UpdateTargetBitrate(
            loss_based_target_rate.bps());
    pushback_rate = std::cmp::max<i64>(self.bandwidth_estimation.GetMinBitrate(),
                                      pushback_rate);
    pushback_target_rate = DataRate::BitsPerSec(pushback_rate);
    if (self.rate_control_settings.UseCongestionWindowDropFrameOnly()) {
      cwnd_reduce_ratio = static_cast<f64>(loss_based_target_rate.bps() -
                                              pushback_target_rate.bps()) /
                          loss_based_target_rate.bps();
    }
  }
  DataRate stable_target_rate =
      self.bandwidth_estimation.GetEstimatedLinkCapacity();
  stable_target_rate = std::cmp::min(stable_target_rate, pushback_target_rate);

  if ((loss_based_target_rate != self.last_loss_based_target_rate) ||
      (loss_based_state != self.last_loss_base_state) ||
      (fraction_loss != self.last_estimated_fraction_loss) ||
      (round_trip_time != self.last_estimated_round_trip_time) ||
      (pushback_target_rate != self.last_pushback_target_rate) ||
      (stable_target_rate != self.last_stable_target_rate)) {
    self.last_loss_based_target_rate = loss_based_target_rate;
    self.last_pushback_target_rate = pushback_target_rate;
    self.last_estimated_fraction_loss = fraction_loss;
    self.last_estimated_round_trip_time = round_trip_time;
    self.last_stable_target_rate = stable_target_rate;
    self.last_loss_base_state = loss_based_state;

    self.alr_detector.SetEstimatedBitrate(loss_based_target_rate.bps());

    TimeDelta bwe_period = self.delay_based_bwe.GetExpectedBwePeriod();

    TargetTransferRate target_rate_msg;
    target_rate_msg.at_time = at_time;
    if (self.rate_control_settings.UseCongestionWindowDropFrameOnly()) {
      target_rate_msg.target_rate = loss_based_target_rate;
      target_rate_msg.cwnd_reduce_ratio = cwnd_reduce_ratio;
    } else {
      target_rate_msg.target_rate = pushback_target_rate;
    }
    target_rate_msg.stable_target_rate = stable_target_rate;
    target_rate_msg.network_estimate.at_time = at_time;
    target_rate_msg.network_estimate.round_trip_time = round_trip_time;
    target_rate_msg.network_estimate.loss_rate_ratio = fraction_loss / 255.0f;
    target_rate_msg.network_estimate.bwe_period = bwe_period;

    update->target_rate = target_rate_msg;

let probes = self.probe_controller.SetEstimatedBitrate(
        loss_based_target_rate,
        GetBandwidthLimitedCause(self.bandwidth_estimation.loss_based_state(),
                                 self.bandwidth_estimation.IsRttAboveLimit(),
                                 self.delay_based_bwe.last_state()),
        at_time);
    update->probe_cluster_configs.insert(update->probe_cluster_configs.end(),
                                         probes.begin(), probes.end());
    update->pacer_config = GetPacingRates(at_time);
    RTC_LOG(LS_VERBOSE) << "bwe " << at_time.ms() << " pushback_target_bps="
                        << self.last_pushback_target_rate.bps()
                        << " estimate_bps=" << loss_based_target_rate.bps();
  }
}

PacerConfig GetPacingRates(&self /* GoogCcNetworkController */,Timestamp at_time) {
  // Pacing rate is based on target rate before congestion window pushback,
  // because we don't want to build queues in the pacer when pushback occurs.
  DataRate pacing_rate = DataRate::Zero();
  if (self.pace_at_max_of_bwe_and_lower_link_capacity && self.estimate &&
      !self.bandwidth_estimation.PaceAtLossBasedEstimate()) {
    pacing_rate =
        std::cmp::max({self.min_total_allocated_bitrate, self.estimate.link_capacity_lower,
                  last_loss_based_target_rate_}) *
        self.pacing_factor;
  } else {
    pacing_rate =
        std::cmp::max(self.min_total_allocated_bitrate, self.last_loss_based_target_rate) *
        self.pacing_factor;
  }
  if (self.limit_pacingfactor_by_upper_link_capacity_estimate && self.estimate &&
      self.estimate.link_capacity_upper.IsFinite() &&
      pacing_rate > self.estimate.link_capacity_upper) {
    pacing_rate =
        std::cmp::max({self.estimate.link_capacity_upper, self.min_total_allocated_bitrate,
                  last_loss_based_target_rate_});
  }

  DataRate padding_rate =
      (self.last_loss_base_state == LossBasedState::kIncreaseUsingPadding)
          ? std::cmp::max(self.max_padding_rate, self.last_loss_based_target_rate)
          max_padding_rate: :,
  padding_rate = std::cmp::min(padding_rate, self.last_pushback_target_rate);
  PacerConfig msg;
  msg.at_time = at_time;
  msg.time_window = Duration::from_secs(1);
  msg.data_window = pacing_rate * msg.time_window;
  msg.pad_window = padding_rate * msg.time_window;
  return msg;
}

}  // namespace webrtc
