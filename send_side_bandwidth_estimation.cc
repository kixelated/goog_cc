/*
 *  Copyright (c) 2012 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/send_side_bandwidth_estimation.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <limits>
#include <memory>
#include <optional>
#include <string>
#include <utility>

#include "api/field_trials_view.h"
#include "api/rtc_event_log/rtc_event_log.h"
#include "api/transport/bandwidth_usage.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/events/rtc_event_bwe_update_loss_based.h"
#include "modules/congestion_controller/goog_cc/loss_based_bwe_v2.h"
#include "modules/remote_bitrate_estimator/include/bwe_defines.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/field_trial_parser.h"
#include "rtc_base/logging.h"
#include "system_wrappers/include/metrics.h"


namespace {
const BweIncreaseInterval: TimeDelta = TimeDelta::Millis(1000);
const BweDecreaseInterval: TimeDelta = TimeDelta::Millis(300);
const StartPhase: TimeDelta = TimeDelta::Millis(2000);
const BweConverganceTime: TimeDelta = TimeDelta::Millis(20000);
const LimitNumPackets: isize = 20;
const DefaultMaxBitrate: DataRate = DataRate::BitsPerSec(1000000000);
const LowBitrateLogPeriod: TimeDelta = TimeDelta::Millis(10000);
const RtcEventLogPeriod: TimeDelta = TimeDelta::Millis(5000);
// Expecting that RTCP feedback is sent uniformly within [0.5, 1.5]s intervals.
const MaxRtcpFeedbackInterval: TimeDelta = TimeDelta::Millis(5000);

const DefaultLowLossThreshold: f32 = 0.02f;
const DefaultHighLossThreshold: f32 = 0.1f;
const DefaultBitrateThreshold: DataRate = DataRate::Zero();

struct UmaRampUpMetric {
  const char* metric_name;
  isize bitrate_kbps;
};

const UmaRampUpMetric kUmaRampupMetrics[] = {
    {"WebRTC.BWE.RampUpTimeTo500kbpsInMs", 500},
    {"WebRTC.BWE.RampUpTimeTo1000kbpsInMs", 1000},
    {"WebRTC.BWE.RampUpTimeTo2000kbpsInMs", 2000}};
const NumUmaRampupMetrics: usize =
    sizeof(kUmaRampupMetrics) / sizeof(kUmaRampupMetrics[0]);

const kBweLosExperiment: &'static str = "WebRTC-BweLossExperiment";

fn BweLossExperimentIsEnabled(const FieldTrialsView& field_trials) -> bool {
  return field_trials.IsEnabled(kBweLosExperiment);
}

bool ReadBweLossExperimentParameters(const FieldTrialsView& field_trials,
                                     f32* low_loss_threshold,
                                     f32* high_loss_threshold,
                                     u32* bitrate_threshold_kbps) {
  assert!(low_loss_threshold);
  assert!(high_loss_threshold);
  assert!(bitrate_threshold_kbps);
  std::string experiment_string = field_trials.Lookup(kBweLosExperiment);
  let parsed_values: isize =
      sscanf(experiment_string.c_str(), "Enabled-%f,%f,%u", low_loss_threshold,
             high_loss_threshold, bitrate_threshold_kbps);
  if (parsed_values == 3) {
    RTC_CHECK_GT(*low_loss_threshold, 0.0)
        << "Loss threshold must be greater than 0.";
    RTC_CHECK_LE(*low_loss_threshold, 1.0)
        << "Loss threshold must be less than or equal to 1.";
    RTC_CHECK_GT(*high_loss_threshold, 0.0)
        << "Loss threshold must be greater than 0.";
    RTC_CHECK_LE(*high_loss_threshold, 1.0)
        << "Loss threshold must be less than or equal to 1.";
    RTC_CHECK_LE(*low_loss_threshold, *high_loss_threshold)
        << "The low loss threshold must be less than or equal to the high loss "
           "threshold.";
    RTC_CHECK_GE(*bitrate_threshold_kbps, 0)
        << "Bitrate threshold can't be negative.";
    RTC_CHECK_LT(*bitrate_threshold_kbps,
                 std::numeric_limits<isize>::max() / 1000)
        << "Bitrate must be smaller enough to avoid overflows.";
    return true;
  }
  RTC_LOG(LS_WARNING) << "Failed to parse parameters for BweLossExperiment "
                         "experiment from field trial string. Using default.";
  *low_loss_threshold = kDefaultLowLossThreshold;
  *high_loss_threshold = kDefaultHighLossThreshold;
  *bitrate_threshold_kbps = kDefaultBitrateThreshold.kbps();
  return false;
}
}  // namespace

fn UpdateDelayBasedEstimate(&self /* LinkCapacityTracker */,
    Timestamp at_time,
    DataRate delay_based_bitrate) {
  if (delay_based_bitrate < self.last_delay_based_estimate) {
    self.capacity_estimate_bps =
        std::cmp::min(self.capacity_estimate_bps, delay_based_bitrate.bps<f64>());
    self.last_link_capacity_update = at_time;
  }
  self.last_delay_based_estimate = delay_based_bitrate;
}

fn OnStartingRate(&self /* LinkCapacityTracker */,DataRate start_rate) {
  if (self.last_link_capacity_update.IsInfinite())
    self.capacity_estimate_bps = start_rate.bps<f64>();
}

fn OnRateUpdate(&self /* LinkCapacityTracker */,Option<DataRate> acknowledged,
                                       DataRate target,
                                       Timestamp at_time) {
  if (!acknowledged)
    return;
  let acknowledged_target: DataRate = std::cmp::min(*acknowledged, target);
  if (acknowledged_target.bps() > self.capacity_estimate_bps) {
    let delta: TimeDelta = at_time - self.last_link_capacity_update;
    let alpha: f64 =
        delta.IsFinite() ? exp(-(delta / Duration::from_secs(10))) : 0;
    self.capacity_estimate_bps = alpha * self.capacity_estimate_bps +
                             (1 - alpha) * acknowledged_target.bps<f64>();
  }
  self.last_link_capacity_update = at_time;
}

fn OnRttBackoff(&self /* LinkCapacityTracker */,DataRate backoff_rate,
                                       Timestamp at_time) {
  self.capacity_estimate_bps =
      std::cmp::min(self.capacity_estimate_bps, backoff_rate.bps<f64>());
  self.last_link_capacity_update = at_time;
}

DataRate estimate(&self /* LinkCapacityTracker */) {
  return DataRate::BitsPerSec(self.capacity_estimate_bps);
}

RttBasedBackoff::RttBasedBackoff(const FieldTrialsView* key_value_config)
    : disabled_("Disabled"),
      configured_limit_("limit", Duration::from_secs(3)),
      drop_fraction_("fraction", 0.8),
      drop_interval_("interval", Duration::from_secs(1)),
      bandwidth_floor_("floor", DataRate::KilobitsPerSec(5)),
      rtt_limit_(TimeDelta::PlusInfinity()),
      // By initializing this to plus infinity, we make sure that we never
      // trigger rtt backoff unless packet feedback is enabled.
      last_propagation_rtt_update_(Timestamp::PlusInfinity()),
      last_propagation_rtt_(TimeDelta::Zero()),
      last_packet_sent_(Timestamp::MinusInfinity()) {
  ParseFieldTrial({&self.disabled, &self.configured_limit, &self.drop_fraction,
                   &self.drop_interval, &bandwidth_floor_},
                  key_value_config->Lookup("WebRTC-Bwe-MaxRttLimit"));
  if (!self.disabled) {
    self.rtt_limit = self.configured_limit.Get();
  }
}

fn UpdatePropagationRtt(&self /* RttBasedBackoff */,Timestamp at_time,
                                           TimeDelta propagation_rtt) {
  self.last_propagation_rtt_update = at_time;
  self.last_propagation_rtt = propagation_rtt;
}

bool IsRttAboveLimit(&self /* RttBasedBackoff */) {
  return CorrectedRtt() > self.rtt_limit;
}

TimeDelta CorrectedRtt(&self /* RttBasedBackoff */) {
  // Avoid timeout when no packets are being sent.
  let timeout_correction: TimeDelta = std::cmp::max(
      self.last_packet_sent - self.last_propagation_rtt_update, TimeDelta::Zero());
  return timeout_correction + self.last_propagation_rtt;
}

RttBasedBackoff::~RttBasedBackoff() = default;

SendSideBandwidthEstimation::SendSideBandwidthEstimation(
    const FieldTrialsView* key_value_config, RtcEventLog* event_log)
    : key_value_config_(key_value_config),
      rtt_backoff_(key_value_config),
      lost_packets_since_last_loss_update_(0),
      expected_packets_since_last_loss_update_(0),
      current_target_(DataRate::Zero()),
      last_logged_target_(DataRate::Zero()),
      min_bitrate_configured_(kCongestionControllerMinBitrate),
      max_bitrate_configured_(kDefaultMaxBitrate),
      last_low_bitrate_log_(Timestamp::MinusInfinity()),
      has_decreased_since_last_fraction_loss_(false),
      last_loss_feedback_(Timestamp::MinusInfinity()),
      last_loss_packet_report_(Timestamp::MinusInfinity()),
      last_fraction_loss_(0),
      last_logged_fraction_loss_(0),
      last_round_trip_time_(TimeDelta::Zero()),
      receiver_limit_(DataRate::PlusInfinity()),
      delay_based_limit_(DataRate::PlusInfinity()),
      time_last_decrease_(Timestamp::MinusInfinity()),
      first_report_time_(Timestamp::MinusInfinity()),
      initially_lost_packets_(0),
      bitrate_at_2_seconds_(DataRate::Zero()),
      uma_update_state_(kNoUpdate),
      uma_rtt_state_(kNoUpdate),
      rampup_uma_stats_updated_(kNumUmaRampupMetrics, false),
      event_log_(event_log),
      last_rtc_event_log_(Timestamp::MinusInfinity()),
      low_loss_threshold_(kDefaultLowLossThreshold),
      high_loss_threshold_(kDefaultHighLossThreshold),
      bitrate_threshold_(kDefaultBitrateThreshold),
      loss_based_bandwidth_estimator_v1_(key_value_config),
      loss_based_bandwidth_estimator_v2_(new LossBasedBweV2(key_value_config)),
      loss_based_state_(LossBasedState::kDelayBasedEstimate),
      disable_receiver_limit_caps_only_("Disabled") {
  assert!(event_log);
  if (BweLossExperimentIsEnabled(*self.key_value_config)) {
    u32 bitrate_threshold_kbps;
    if (ReadBweLossExperimentParameters(
            *self.key_value_config, &self.low_loss_threshold, &self.high_loss_threshold,
            &bitrate_threshold_kbps)) {
      RTC_LOG(LS_INFO) << "Enabled BweLossExperiment with parameters "
                       << self.low_loss_threshold << ", " << high_loss_threshold_
                       << ", " << bitrate_threshold_kbps;
      self.bitrate_threshold = DataRate::KilobitsPerSec(bitrate_threshold_kbps);
    }
  }
  ParseFieldTrial({&disable_receiver_limit_caps_only_},
                  key_value_config->Lookup("WebRTC-Bwe-ReceiverLimitCapsOnly"));
  if (LossBasedBandwidthEstimatorV2Enabled()) {
    self.loss_based_bandwidth_estimator_v2.SetMinMaxBitrate(
        self.min_bitrate_configured, self.max_bitrate_configured);
  }
}

SendSideBandwidthEstimation::~SendSideBandwidthEstimation() {}

fn OnRouteChange(&self /* SendSideBandwidthEstimation */) {
  self.lost_packets_since_last_loss_update = 0;
  self.expected_packets_since_last_loss_update = 0;
  self.current_target = DataRate::Zero();
  self.min_bitrate_configured = kCongestionControllerMinBitrate;
  self.max_bitrate_configured = kDefaultMaxBitrate;
  self.last_low_bitrate_log = Timestamp::MinusInfinity();
  self.has_decreased_since_last_fraction_loss = false;
  self.last_loss_feedback = Timestamp::MinusInfinity();
  self.last_loss_packet_report = Timestamp::MinusInfinity();
  self.last_fraction_loss = 0;
  self.last_logged_fraction_loss = 0;
  self.last_round_trip_time = TimeDelta::Zero();
  self.receiver_limit = DataRate::PlusInfinity();
  self.delay_based_limit = DataRate::PlusInfinity();
  self.time_last_decrease = Timestamp::MinusInfinity();
  self.first_report_time = Timestamp::MinusInfinity();
  self.initially_lost_packets = 0;
  self.bitrate_at_2_seconds = DataRate::Zero();
  self.uma_update_state = kNoUpdate;
  self.uma_rtt_state = kNoUpdate;
  self.last_rtc_event_log = Timestamp::MinusInfinity();
  if (LossBasedBandwidthEstimatorV2Enabled() &&
      self.loss_based_bandwidth_estimator_v2.UseInStartPhase()) {
    self.loss_based_bandwidth_estimator_v2.reset(
        new LossBasedBweV2(self.key_value_config));
  }
}

fn SetBitrates(&self /* SendSideBandwidthEstimation */,
    Option<DataRate> send_bitrate,
    DataRate min_bitrate,
    DataRate max_bitrate,
    Timestamp at_time) {
  SetMinMaxBitrate(min_bitrate, max_bitrate);
  if (send_bitrate) {
    self.link_capacity.OnStartingRate(*send_bitrate);
    SetSendBitrate(*send_bitrate, at_time);
  }
}

fn SetSendBitrate(&self /* SendSideBandwidthEstimation */,DataRate bitrate,
                                                 Timestamp at_time) {
  assert!_GT(bitrate, DataRate::Zero());
  // Reset to avoid being capped by the estimate.
  self.delay_based_limit = DataRate::PlusInfinity();
  UpdateTargetBitrate(bitrate, at_time);
  // Clear last sent bitrate history so the new value can be used directly
  // and not capped.
  self.min_bitrate_history.clear();
}

fn SetMinMaxBitrate(&self /* SendSideBandwidthEstimation */,DataRate min_bitrate,
                                                   DataRate max_bitrate) {
  self.min_bitrate_configured =
      std::cmp::max(min_bitrate, kCongestionControllerMinBitrate);
  if (max_bitrate > DataRate::Zero() && max_bitrate.IsFinite()) {
    self.max_bitrate_configured = std::cmp::max(self.min_bitrate_configured, max_bitrate);
  } else {
    self.max_bitrate_configured = kDefaultMaxBitrate;
  }
  self.loss_based_bandwidth_estimator_v2.SetMinMaxBitrate(self.min_bitrate_configured,
                                                       self.max_bitrate_configured);
}

int GetMinBitrate(&self /* SendSideBandwidthEstimation */) {
  return self.min_bitrate_configured.bps<isize>();
}

DataRate target_rate(&self /* SendSideBandwidthEstimation */) {
  let target: DataRate = self.current_target;
  if (!self.disable_receiver_limit_caps_only)
    target = std::cmp::min(target, self.receiver_limit);
  return std::cmp::max(self.min_bitrate_configured, target);
}

LossBasedState loss_based_state(&self /* SendSideBandwidthEstimation */) {
  loss_based_state: return,
}

bool IsRttAboveLimit(&self /* SendSideBandwidthEstimation */) {
  return self.rtt_backoff.IsRttAboveLimit();
}

DataRate GetEstimatedLinkCapacity(&self /* SendSideBandwidthEstimation */) {
  return self.link_capacity.estimate();
}

fn UpdateReceiverEstimate(&self /* SendSideBandwidthEstimation */,Timestamp at_time,
                                                         DataRate bandwidth) {
  // TODO(srte): Ensure caller passes PlusInfinity, not zero, to represent no
  // limitation.
  self.receiver_limit = bandwidth.IsZero() ? DataRate::PlusInfinity() : bandwidth;
  ApplyTargetLimits(at_time);
}

fn UpdateDelayBasedEstimate(&self /* SendSideBandwidthEstimation */,Timestamp at_time,
                                                           DataRate bitrate) {
  self.link_capacity.UpdateDelayBasedEstimate(at_time, bitrate);
  // TODO(srte): Ensure caller passes PlusInfinity, not zero, to represent no
  // limitation.
  self.delay_based_limit = bitrate.IsZero() ? DataRate::PlusInfinity() : bitrate;
  ApplyTargetLimits(at_time);
}

fn SetAcknowledgedRate(&self /* SendSideBandwidthEstimation */,
    Option<DataRate> acknowledged_rate,
    Timestamp at_time) {
  self.acknowledged_rate = acknowledged_rate;
  if (!acknowledged_rate.is_some()) {
    return;
  }
  if (LossBasedBandwidthEstimatorV1Enabled()) {
    self.loss_based_bandwidth_estimator_v1.UpdateAcknowledgedBitrate(
        *acknowledged_rate, at_time);
  }
  if (LossBasedBandwidthEstimatorV2Enabled()) {
    self.loss_based_bandwidth_estimator_v2.SetAcknowledgedBitrate(
        *acknowledged_rate);
  }
}

fn UpdateLossBasedEstimator(&self /* SendSideBandwidthEstimation */,
    const TransportPacketsFeedback& report,
    BandwidthUsage /* delay_detector_state */,
    Option<DataRate> /* probe_bitrate */,
    bool in_alr) {
  if (LossBasedBandwidthEstimatorV1Enabled()) {
    self.loss_based_bandwidth_estimator_v1.UpdateLossStatistics(
        report.packet_feedbacks, report.feedback_time);
  }
  if (LossBasedBandwidthEstimatorV2Enabled()) {
    self.loss_based_bandwidth_estimator_v2.UpdateBandwidthEstimate(
        report.packet_feedbacks, self.delay_based_limit, in_alr);
    UpdateEstimate(report.feedback_time);
  }
}

fn UpdatePacketsLost(&self /* SendSideBandwidthEstimation */,i64 packets_lost,
                                                    i64 number_of_packets,
                                                    Timestamp at_time) {
  self.last_loss_feedback = at_time;
  if (self.first_report_time.IsInfinite())
    self.first_report_time = at_time;

  // Check sequence number diff and weight loss report
  if (number_of_packets > 0) {
    let expected: i64 =
        self.expected_packets_since_last_loss_update + number_of_packets;

    // Don't generate a loss rate until it can be based on enough packets.
    if (expected < kLimitNumPackets) {
      // Accumulate reports.
      self.expected_packets_since_last_loss_update = expected;
      self.lost_packets_since_last_loss_update += packets_lost;
      return;
    }

    self.has_decreased_since_last_fraction_loss = false;
    let lost_q8: i64 =
        std::cmp::max<i64>(self.lost_packets_since_last_loss_update + packets_lost,
                          0)
        << 8;
    self.last_fraction_loss = std::cmp::min<isize>(lost_q8 / expected, 255);

    // Reset accumulators.
    self.lost_packets_since_last_loss_update = 0;
    self.expected_packets_since_last_loss_update = 0;
    self.last_loss_packet_report = at_time;
    UpdateEstimate(at_time);
  }

  UpdateUmaStatsPacketsLost(at_time, packets_lost);
}

fn UpdateUmaStatsPacketsLost(&self /* SendSideBandwidthEstimation */,Timestamp at_time,
                                                            isize packets_lost) {
  let bitrate_kbps: DataRate =
      DataRate::KilobitsPerSec((self.current_target.bps() + 500) / 1000);
  let i: for = 0; i < kNumUmaRampupMetricsi += 1) {
    if (!self.rampup_uma_stats_updated[i] &&
        bitrate_kbps.kbps() >= kUmaRampupMetrics[i].bitrate_kbps) {
      RTC_HISTOGRAMS_COUNTS_100000(i, kUmaRampupMetrics[i].metric_name,
                                   (at_time - self.first_report_time).ms());
      self.rampup_uma_stats_updated[i] = true;
    }
  }
  if (IsInStartPhase(at_time)) {
    self.initially_lost_packets += packets_lost;
  } else if (self.uma_update_state == kNoUpdate) {
    self.uma_update_state = kFirstDone;
    self.bitrate_at_2_seconds = bitrate_kbps;
    RTC_HISTOGRAM_COUNTS("WebRTC.BWE.InitiallyLostPackets",
                         self.initially_lost_packets, 0, 100, 50);
    RTC_HISTOGRAM_COUNTS("WebRTC.BWE.InitialBandwidthEstimate",
                         self.bitrate_at_2_seconds.kbps(), 0, 2000, 50);
  } else if (self.uma_update_state == kFirstDone &&
             at_time - self.first_report_time >= kBweConverganceTime) {
    self.uma_update_state = kDone;
    let bitrate_diff_kbps: isize = std::cmp::max(
        self.bitrate_at_2_seconds.kbps<isize>() - bitrate_kbps.kbps<isize>(), 0);
    RTC_HISTOGRAM_COUNTS("WebRTC.BWE.InitialVsConvergedDiff", bitrate_diff_kbps,
                         0, 2000, 50);
  }
}

fn UpdateRtt(&self /* SendSideBandwidthEstimation */,TimeDelta rtt, Timestamp at_time) {
  // Update RTT if we were able to compute an RTT based on this RTCP.
  // FlexFEC doesn't send RTCP SR, which means we won't be able to compute RTT.
  if (rtt > TimeDelta::Zero())
    self.last_round_trip_time = rtt;

  if (!IsInStartPhase(at_time) && self.uma_rtt_state == kNoUpdate) {
    self.uma_rtt_state = kDone;
    RTC_HISTOGRAM_COUNTS("WebRTC.BWE.InitialRtt", rtt.ms<isize>(), 0, 2000, 50);
  }
}

fn UpdateEstimate(&self /* SendSideBandwidthEstimation */,Timestamp at_time) {
  if (self.rtt_backoff.IsRttAboveLimit()) {
    if (at_time - self.time_last_decrease >= self.rtt_backoff.self.drop_interval &&
        self.current_target > self.rtt_backoff.self.bandwidth_floor) {
      self.time_last_decrease = at_time;
      let new_bitrate: DataRate =
          std::cmp::max(self.current_target * self.rtt_backoff.self.drop_fraction,
                   self.rtt_backoff.self.bandwidth_floor.Get());
      self.link_capacity.OnRttBackoff(new_bitrate, at_time);
      UpdateTargetBitrate(new_bitrate, at_time);
      return;
    }
    // TODO(srte): This is likely redundant in most cases.
    ApplyTargetLimits(at_time);
    return;
  }

  // We trust the REMB and/or delay-based estimate during the first 2 seconds if
  // we haven't had any packet loss reported, to allow startup bitrate probing.
  if (self.last_fraction_loss == 0 && IsInStartPhase(at_time) &&
      !self.loss_based_bandwidth_estimator_v2.ReadyToUseInStartPhase()) {
    let new_bitrate: DataRate = self.current_target;
    // TODO(srte): We should not allow the new_bitrate to be larger than the
    // receiver limit here.
    if (self.receiver_limit.IsFinite())
      new_bitrate = std::cmp::max(self.receiver_limit, new_bitrate);
    if (self.delay_based_limit.IsFinite())
      new_bitrate = std::cmp::max(self.delay_based_limit, new_bitrate);
    if (LossBasedBandwidthEstimatorV1Enabled()) {
      self.loss_based_bandwidth_estimator_v1.Initialize(new_bitrate);
    }

    if (new_bitrate != self.current_target) {
      self.min_bitrate_history.clear();
      if (LossBasedBandwidthEstimatorV1Enabled()) {
        self.min_bitrate_history.push_back(std::make_pair(at_time, new_bitrate));
      } else {
        self.min_bitrate_history.push_back(
            std::make_pair(at_time, self.current_target));
      }
      UpdateTargetBitrate(new_bitrate, at_time);
      return;
    }
  }
  UpdateMinHistory(at_time);
  if (self.last_loss_packet_report.IsInfinite()) {
    // No feedback received.
    // TODO(srte): This is likely redundant in most cases.
    ApplyTargetLimits(at_time);
    return;
  }

  if (LossBasedBandwidthEstimatorV1ReadyForUse()) {
    let new_bitrate: DataRate = self.loss_based_bandwidth_estimator_v1.Update(
        at_time, self.min_bitrate_history.front().second, self.delay_based_limit,
        self.last_round_trip_time);
    UpdateTargetBitrate(new_bitrate, at_time);
    return;
  }

  if (LossBasedBandwidthEstimatorV2ReadyForUse()) {
    LossBasedBweV2::Result result =
        self.loss_based_bandwidth_estimator_v2.GetLossBasedResult();
    self.loss_based_state = result.state;
    UpdateTargetBitrate(result.bandwidth_estimate, at_time);
    return;
  }

  let time_since_loss_packet_report: TimeDelta = at_time - self.last_loss_packet_report;
  if (time_since_loss_packet_report < 1.2 * kMaxRtcpFeedbackInterval) {
    // We only care about loss above a given bitrate threshold.
    let loss: f32 = self.last_fraction_loss / 256.0;
    // We only make decisions based on loss when the bitrate is above a
    // threshold. This is a crude way of handling loss which is uncorrelated
    // to congestion.
    if (self.current_target < self.bitrate_threshold || loss <= self.low_loss_threshold) {
      // Loss < 2%: Increase rate by 8% of the min bitrate in the last
      // kBweIncreaseInterval.
      // Note that by remembering the bitrate over the last second one can
      // rampup up one second faster than if only allowed to start ramping
      // at 8% per second rate now. E.g.:
      //   If sending a constant 100kbps it can rampup immediately to 108kbps
      //   whenever a receiver report is received with lower packet loss.
      //   If instead one would do: self.current_bitrate *= 1.08^(delta time),
      //   it would take over one second since the lower packet loss to achieve
      //   108kbps.
      let new_bitrate: DataRate = DataRate::BitsPerSec(
          self.min_bitrate_history.front().second.bps() * 1.08 + 0.5);

      // Add 1 kbps extra, just to make sure that we do not get stuck
      // (gives a little extra increase at low rates, negligible at higher
      // rates).
      new_bitrate += DataRate::BitsPerSec(1000);
      UpdateTargetBitrate(new_bitrate, at_time);
      return;
    } else if (self.current_target > self.bitrate_threshold) {
      if (loss <= self.high_loss_threshold) {
        // Loss between 2% - 10%: Do nothing.
      } else {
        // Loss > 10%: Limit the rate decreases to once a kBweDecreaseInterval
        // + rtt.
        if (!self.has_decreased_since_last_fraction_loss &&
            (at_time - self.time_last_decrease) >=
                (kBweDecreaseInterval + self.last_round_trip_time)) {
          self.time_last_decrease = at_time;

          // Reduce rate:
          //   newRate = rate * (1 - 0.5*lossRate);
          //   where packetLoss = 256*lossRate;
          let new_bitrate: DataRate = DataRate::BitsPerSec(
              (self.current_target.bps() *
               (512 - self.last_fraction_loss) as f64) /
              512.0);
          self.has_decreased_since_last_fraction_loss = true;
          UpdateTargetBitrate(new_bitrate, at_time);
          return;
        }
      }
    }
  }
  // TODO(srte): This is likely redundant in most cases.
  ApplyTargetLimits(at_time);
}

fn UpdatePropagationRtt(&self /* SendSideBandwidthEstimation */,
    Timestamp at_time,
    TimeDelta propagation_rtt) {
  self.rtt_backoff.UpdatePropagationRtt(at_time, propagation_rtt);
}

fn OnSentPacket(&self /* SendSideBandwidthEstimation */,const SentPacket& sent_packet) {
  // Only feedback-triggering packets will be reported here.
  self.rtt_backoff.self.last_packet_sent = sent_packet.send_time;
}

bool IsInStartPhase(&self /* SendSideBandwidthEstimation */,Timestamp at_time) {
  return self.first_report_time.IsInfinite() ||
         at_time - self.first_report_time < kStartPhase;
}

fn UpdateMinHistory(&self /* SendSideBandwidthEstimation */,Timestamp at_time) {
  // Remove old data points from history.
  // Since history precision is in ms, add one so it is able to increase
  // bitrate if it is off by as little as 0.5ms.
  while (!self.min_bitrate_history.empty() &&
         at_time - self.min_bitrate_history.front().first + TimeDelta::Millis(1) >
             kBweIncreaseInterval) {
    self.min_bitrate_history.pop_front();
  }

  // Typical minimum sliding-window algorithm: Pop values higher than current
  // bitrate before pushing it.
  while (!self.min_bitrate_history.empty() &&
         self.current_target <= self.min_bitrate_history.back().second) {
    self.min_bitrate_history.pop_back();
  }

  self.min_bitrate_history.push_back(std::make_pair(at_time, self.current_target));
}

DataRate GetUpperLimit(&self /* SendSideBandwidthEstimation */) {
  let upper_limit: DataRate = self.delay_based_limit;
  if (self.disable_receiver_limit_caps_only)
    upper_limit = std::cmp::min(upper_limit, self.receiver_limit);
  return std::cmp::min(upper_limit, self.max_bitrate_configured);
}

fn MaybeLogLowBitrateWarning(&self /* SendSideBandwidthEstimation */,DataRate bitrate,
                                                            Timestamp at_time) {
  if (at_time - self.last_low_bitrate_log > kLowBitrateLogPeriod) {
    RTC_LOG(LS_WARNING) << "Estimated available bandwidth " << ToString(bitrate)
                        << " is below configured min bitrate "
                        << ToString(self.min_bitrate_configured) << ".";
    self.last_low_bitrate_log = at_time;
  }
}

fn MaybeLogLossBasedEvent(&self /* SendSideBandwidthEstimation */,Timestamp at_time) {
  if (current_target_ != self.last_logged_target ||
      last_fraction_loss_ != self.last_logged_fraction_loss ||
      at_time - self.last_rtc_event_log > kRtcEventLogPeriod) {
    self.event_log.Log(std::make_unique<RtcEventBweUpdateLossBased>(
        self.current_target.bps(), self.last_fraction_loss,
        self.expected_packets_since_last_loss_update));
    self.last_logged_fraction_loss = self.last_fraction_loss;
    self.last_logged_target = self.current_target;
    self.last_rtc_event_log = at_time;
  }
}

fn UpdateTargetBitrate(&self /* SendSideBandwidthEstimation */,DataRate new_bitrate,
                                                      Timestamp at_time) {
  new_bitrate = std::cmp::min(new_bitrate, GetUpperLimit());
  if (new_bitrate < self.min_bitrate_configured) {
    MaybeLogLowBitrateWarning(new_bitrate, at_time);
    new_bitrate = self.min_bitrate_configured;
  }
  self.current_target = new_bitrate;
  MaybeLogLossBasedEvent(at_time);
  self.link_capacity.OnRateUpdate(self.acknowledged_rate, self.current_target, at_time);
}

fn ApplyTargetLimits(&self /* SendSideBandwidthEstimation */,Timestamp at_time) {
  UpdateTargetBitrate(self.current_target, at_time);
}

bool LossBasedBandwidthEstimatorV1Enabled(&self /* SendSideBandwidthEstimation */) {
  return self.loss_based_bandwidth_estimator_v1.Enabled() &&
         !LossBasedBandwidthEstimatorV2Enabled();
}

bool LossBasedBandwidthEstimatorV1ReadyForUse(&self /* SendSideBandwidthEstimation */)
    {
  return LossBasedBandwidthEstimatorV1Enabled() &&
         self.loss_based_bandwidth_estimator_v1.InUse();
}

bool LossBasedBandwidthEstimatorV2Enabled(&self /* SendSideBandwidthEstimation */) {
  return self.loss_based_bandwidth_estimator_v2.IsEnabled();
}

bool LossBasedBandwidthEstimatorV2ReadyForUse(&self /* SendSideBandwidthEstimation */)
    {
  return self.loss_based_bandwidth_estimator_v2.IsReady();
}

bool PaceAtLossBasedEstimate(&self /* SendSideBandwidthEstimation */) {
  return LossBasedBandwidthEstimatorV2ReadyForUse() &&
         self.loss_based_bandwidth_estimator_v2.PaceAtLossBasedEstimate();
}

}  // namespace webrtc
