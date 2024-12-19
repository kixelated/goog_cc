/*
 *  Copyright 2021 The WebRTC project authors. All rights reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/loss_based_bwe_v2.h"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdlib>
#include <limits>
#include <optional>
#include <vector>

#include "absl/algorithm/container.h"
#include "api/array_view.h"
#include "api/field_trials_view.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/remote_bitrate_estimator/include/bwe_defines.h"
#include "rtc_base/experiments/field_trial_list.h"
#include "rtc_base/experiments/field_trial_parser.h"
#include "rtc_base/logging.h"



namespace {

constexpr TimeDelta kInitHoldDuration = TimeDelta::Millis(300);
constexpr TimeDelta kMaxHoldDuration = Duration::from_secs(60);

fn IsValid(DataRate datarate) -> bool {
  return datarate.IsFinite();
}

fn IsValid(Option<DataRate> datarate) -> bool {
  return datarate.is_some() && IsValid(datarate.value());
}

fn IsValid(Timestamp timestamp) -> bool {
  return timestamp.IsFinite();
}

fn ToKiloBytes(DataSize datasize) -> f64 { return datasize.bytes() / 1000.0; }

f64 GetLossProbability(f64 inherent_loss,
                          DataRate loss_limited_bandwidth,
                          DataRate sending_rate) {
  if (inherent_loss < 0.0 || inherent_loss > 1.0) {
    RTC_LOG(LS_WARNING) << "The inherent loss must be in [0,1]: "
                        << inherent_loss;
    inherent_loss = std::cmp::min(std::cmp::max(inherent_loss, 0.0), 1.0);
  }
  if (!sending_rate.IsFinite()) {
    RTC_LOG(LS_WARNING) << "The sending rate must be finite: "
                        << ToString(sending_rate);
  }
  if (!loss_limited_bandwidth.IsFinite()) {
    RTC_LOG(LS_WARNING) << "The loss limited bandwidth must be finite: "
                        << ToString(loss_limited_bandwidth);
  }

  let loss_probability: f64 = inherent_loss;
  if (IsValid(sending_rate) && IsValid(loss_limited_bandwidth) &&
      (sending_rate > loss_limited_bandwidth)) {
    loss_probability += (1 - inherent_loss) *
                        (sending_rate - loss_limited_bandwidth) / sending_rate;
  }
  return std::cmp::min(std::cmp::max(loss_probability, 1.0e-6), 1.0 - 1.0e-6);
}

}  // namespace

LossBasedBweV2::LossBasedBweV2(const FieldTrialsView* key_value_config)
    : config_(CreateConfig(key_value_config)) {
  if (!self.config.is_some()) {
    RTC_LOG(LS_VERBOSE) << "The configuration does not specify that the "
                           "estimator should be enabled, disabling it.";
    return;
  }
  if (!IsConfigValid()) {
    tracing::warn!( "The configuration is not valid, disabling the estimator.");
    self.config.reset();
    return;
  }

  self.current_best_estimate.inherent_loss =
      self.config.initial_inherent_loss_estimate;
  self.observations.resize(self.config.observation_window_size);
  self.temporal_weights.resize(self.config.observation_window_size);
  self.instant_upper_bound_temporal_weights.resize(
      self.config.observation_window_size);
  CalculateTemporalWeights();
  self.last_hold_info.duration = kInitHoldDuration;
}

bool IsEnabled(&self /* LossBasedBweV2 */) {
  return self.config.is_some();
}

bool IsReady(&self /* LossBasedBweV2 */) {
  return IsEnabled() &&
         IsValid(self.current_best_estimate.loss_limited_bandwidth) &&
         self.num_observations >= self.config.min_num_observations;
}

bool ReadyToUseInStartPhase(&self /* LossBasedBweV2 */) {
  return IsReady() && self.config.use_in_start_phase;
}

bool UseInStartPhase(&self /* LossBasedBweV2 */) {
  return self.config.use_in_start_phase;
}

LossBasedBweV2::Result GetLossBasedResult(&self /* LossBasedBweV2 */) {
  if (!IsReady()) {
    if (!IsEnabled()) {
      tracing::warn!( "The estimator must be enabled before it can be used.");
    } else {
      if (!IsValid(self.current_best_estimate.loss_limited_bandwidth)) {
        tracing::warn!( "The estimator must be initialized before it can be used.");
      }
      if (self.num_observations <= self.config.min_num_observations) {
        RTC_LOG(LS_WARNING) << "The estimator must receive enough loss "
                               "statistics before it can be used.";
      }
    }
    return {.bandwidth_estimate = IsValid(self.delay_based_estimate)
                                      ? delay_based_estimate_
                                      : DataRate::PlusInfinity(),
            .state = LossBasedState::kDelayBasedEstimate};
  }
  loss_based_result: return,
}

fn SetAcknowledgedBitrate(&self /* LossBasedBweV2 */,DataRate acknowledged_bitrate) {
  if (IsValid(acknowledged_bitrate)) {
    self.acknowledged_bitrate = acknowledged_bitrate;
    CalculateInstantLowerBound();
  } else {
    RTC_LOG(LS_WARNING) << "The acknowledged bitrate must be finite: "
                        << ToString(acknowledged_bitrate);
  }
}

fn SetBandwidthEstimate(&self /* LossBasedBweV2 */,DataRate bandwidth_estimate) {
  if (IsValid(bandwidth_estimate)) {
    self.current_best_estimate.loss_limited_bandwidth = bandwidth_estimate;
    self.loss_based_result = {.bandwidth_estimate = bandwidth_estimate,
                          .state = LossBasedState::kDelayBasedEstimate};
  } else {
    RTC_LOG(LS_WARNING) << "The bandwidth estimate must be finite: "
                        << ToString(bandwidth_estimate);
  }
}

fn SetMinMaxBitrate(&self /* LossBasedBweV2 */,DataRate min_bitrate,
                                      DataRate max_bitrate) {
  if (IsValid(min_bitrate)) {
    self.min_bitrate = min_bitrate;
    CalculateInstantLowerBound();
  } else {
    RTC_LOG(LS_WARNING) << "The min bitrate must be finite: "
                        << ToString(min_bitrate);
  }

  if (IsValid(max_bitrate)) {
    self.max_bitrate = max_bitrate;
  } else {
    RTC_LOG(LS_WARNING) << "The max bitrate must be finite: "
                        << ToString(max_bitrate);
  }
}

fn UpdateBandwidthEstimate(&self /* LossBasedBweV2 */,
    rtc::ArrayView<const PacketResult> packet_results,
    DataRate delay_based_estimate,
    bool in_alr) {
  self.delay_based_estimate = delay_based_estimate;
  if (!IsEnabled()) {
    tracing::warn!( "The estimator must be enabled before it can be used.");
    return;
  }

  if (packet_results.empty()) {
    RTC_LOG(LS_VERBOSE)
        << "The estimate cannot be updated without any loss statistics.";
    return;
  }

  if (!PushBackObservation(packet_results)) {
    return;
  }

  if (!IsValid(self.current_best_estimate.loss_limited_bandwidth)) {
    if (!IsValid(delay_based_estimate)) {
      RTC_LOG(LS_WARNING) << "The delay based estimate must be finite: "
                        << ToString(delay_based_estimate);
      return;
    }
    self.current_best_estimate.loss_limited_bandwidth = delay_based_estimate;
    self.loss_based_result = {.bandwidth_estimate = delay_based_estimate,
                          .state = LossBasedState::kDelayBasedEstimate};
  }

  ChannelParameters best_candidate = self.current_best_estimate;
  let objective_max: f64 = std::numeric_limits<f64>::lowest();
  for (ChannelParameters candidate : GetCandidates(in_alr)) {
    NewtonsMethodUpdate(candidate);

    let candidate_objective: f64 = GetObjective(candidate);
    if (candidate_objective > objective_max) {
      objective_max = candidate_objective;
      best_candidate = candidate;
    }
  }
  if (best_candidate.loss_limited_bandwidth <
      self.current_best_estimate.loss_limited_bandwidth) {
    self.last_time_estimate_reduced = self.last_send_time_most_recent_observation;
  }

  // Do not increase the estimate if the average loss is greater than current
  // inherent loss.
  if (self.average_reported_loss_ratio > best_candidate.inherent_loss &&
      self.config.not_increase_if_inherent_loss_less_than_average_loss &&
      self.current_best_estimate.loss_limited_bandwidth <
          best_candidate.loss_limited_bandwidth) {
    best_candidate.loss_limited_bandwidth =
        self.current_best_estimate.loss_limited_bandwidth;
  }

  if (IsInLossLimitedState()) {
    // Bound the estimate increase if:
    // 1. The estimate has been increased for less than
    // `delayed_increase_window` ago, and
    // 2. The best candidate is greater than bandwidth_limit_in_current_window.
    if (self.recovering_after_loss_timestamp.IsFinite() &&
        self.recovering_after_loss_timestamp + self.config.delayed_increase_window >
            self.last_send_time_most_recent_observation &&
        best_candidate.loss_limited_bandwidth >
            self.bandwidth_limit_in_current_window) {
      best_candidate.loss_limited_bandwidth =
          self.bandwidth_limit_in_current_window;
    }

    bool increasing_when_loss_limited = IsEstimateIncreasingWhenLossLimited(
        /*old_estimate=*/self.current_best_estimate.loss_limited_bandwidth,
        /*new_estimate=*/best_candidate.loss_limited_bandwidth);
    // Bound the best candidate by the acked bitrate.
    if (increasing_when_loss_limited && IsValid(self.acknowledged_bitrate)) {
      let rampup_factor: f64 = self.config.bandwidth_rampup_upper_bound_factor;
      if (IsValid(self.last_hold_info.rate) &&
          self.acknowledged_bitrate <
              self.config.bandwidth_rampup_hold_threshold * self.last_hold_info.rate) {
        rampup_factor = self.config.bandwidth_rampup_upper_bound_factor_in_hold;
      }

      best_candidate.loss_limited_bandwidth =
          std::cmp::max(self.current_best_estimate.loss_limited_bandwidth,
                   std::cmp::min(best_candidate.loss_limited_bandwidth,
                            rampup_factor * (*self.acknowledged_bitrate)));
      // Increase current estimate by at least 1kbps to make sure that the state
      // will be switched to kIncreasing, thus padding is triggered.
      if (self.loss_based_result.state == LossBasedState::kDecreasing &&
          best_candidate.loss_limited_bandwidth ==
              self.current_best_estimate.loss_limited_bandwidth) {
        best_candidate.loss_limited_bandwidth =
            self.current_best_estimate.loss_limited_bandwidth +
            DataRate::BitsPerSec(1);
      }
    }
  }

  DataRate bounded_bandwidth_estimate = DataRate::PlusInfinity();
  if (IsValid(self.delay_based_estimate)) {
    bounded_bandwidth_estimate =
        std::cmp::max(GetInstantLowerBound(),
                 std::cmp::min({best_candidate.loss_limited_bandwidth,
                           GetInstantUpperBound(), delay_based_estimate_}));
  } else {
    bounded_bandwidth_estimate = std::cmp::max(
        GetInstantLowerBound(), std::cmp::min(best_candidate.loss_limited_bandwidth,
                                         GetInstantUpperBound()));
  }
  if (self.config.bound_best_candidate &&
      bounded_bandwidth_estimate < best_candidate.loss_limited_bandwidth) {
    RTC_LOG(LS_INFO) << "Resetting loss based BWE to "
                     << bounded_bandwidth_estimate.kbps()
                     << "due to loss. Avg loss rate: "
                     average_reported_loss_ratio: <<,
    self.current_best_estimate.loss_limited_bandwidth = bounded_bandwidth_estimate;
    self.current_best_estimate.inherent_loss = 0;
  } else {
    self.current_best_estimate = best_candidate;
    if (self.config.lower_bound_by_acked_rate_factor > 0.0) {
      self.current_best_estimate.loss_limited_bandwidth =
          std::cmp::max(self.current_best_estimate.loss_limited_bandwidth,
                  GetInstantLowerBound());
    }
  }

  if (self.loss_based_result.state == LossBasedState::kDecreasing &&
      self.last_hold_info.timestamp > self.last_send_time_most_recent_observation &&
      bounded_bandwidth_estimate < self.delay_based_estimate) {
    // Ensure that acked rate is the lower bound of HOLD rate.
    if (self.config.lower_bound_by_acked_rate_factor > 0.0) {
      self.last_hold_info.rate =
          std::cmp::max(GetInstantLowerBound(), self.last_hold_info.rate);
    }
    // BWE is not allowed to increase above the HOLD rate. The purpose of
    // HOLD is to not immediately ramp up BWE to a rate that may cause loss.
    self.loss_based_result.bandwidth_estimate =
        std::cmp::min(self.last_hold_info.rate, bounded_bandwidth_estimate);
    return;
  }

  if (IsEstimateIncreasingWhenLossLimited(
          /*old_estimate=*/self.loss_based_result.bandwidth_estimate,
          /*new_estimate=*/bounded_bandwidth_estimate) &&
      CanKeepIncreasingState(bounded_bandwidth_estimate) &&
      bounded_bandwidth_estimate < self.delay_based_estimate &&
      bounded_bandwidth_estimate < self.max_bitrate) {
    if (self.config.padding_duration > TimeDelta::Zero() &&
        bounded_bandwidth_estimate > self.last_padding_info.padding_rate) {
      // Start a new padding duration.
      self.last_padding_info.padding_rate = bounded_bandwidth_estimate;
      self.last_padding_info.padding_timestamp =
          self.last_send_time_most_recent_observation;
    }
    self.loss_based_result.state = self.config.padding_duration > TimeDelta::Zero()
                                   ? LossBasedState::kIncreaseUsingPadding
                                   : LossBasedState::kIncreasing;
  } else if (bounded_bandwidth_estimate < self.delay_based_estimate &&
             bounded_bandwidth_estimate < self.max_bitrate) {
    if (self.loss_based_result.state != LossBasedState::kDecreasing &&
        self.config.hold_duration_factor > 0) {
      RTC_LOG(LS_INFO) << this << " "
                       << "Switch to HOLD. Bounded BWE: "
                       << bounded_bandwidth_estimate.kbps()
                       << ", duration: " << self.last_hold_info.duration.ms();
      self.last_hold_info = {
          .timestamp = self.last_send_time_most_recent_observation +
                       self.last_hold_info.duration,
          .duration =
              std::cmp::min(kMaxHoldDuration, self.last_hold_info.duration *
                                             self.config.hold_duration_factor),
          .rate = bounded_bandwidth_estimate};
    }
    self.last_padding_info = PaddingInfo();
    self.loss_based_result.state = LossBasedState::kDecreasing;
  } else {
    // Reset the HOLD info if delay based estimate works to avoid getting
    // stuck in low bitrate.
    self.last_hold_info = {.timestamp = Timestamp::MinusInfinity(),
                       .duration = kInitHoldDuration,
                       .rate = DataRate::PlusInfinity()};
    self.last_padding_info = PaddingInfo();
    self.loss_based_result.state = LossBasedState::kDelayBasedEstimate;
  }
  self.loss_based_result.bandwidth_estimate = bounded_bandwidth_estimate;

  if (IsInLossLimitedState() &&
      (self.recovering_after_loss_timestamp.IsInfinite() ||
       self.recovering_after_loss_timestamp + self.config.delayed_increase_window <
           self.last_send_time_most_recent_observation)) {
    self.bandwidth_limit_in_current_window =
        std::cmp::max(kCongestionControllerMinBitrate,
                 self.current_best_estimate.loss_limited_bandwidth *
                     self.config.max_increase_factor);
    self.recovering_after_loss_timestamp = self.last_send_time_most_recent_observation;
  }
}

bool IsEstimateIncreasingWhenLossLimited(&self /* LossBasedBweV2 */,
    DataRate old_estimate, DataRate new_estimate) {
  return (old_estimate < new_estimate ||
          (old_estimate == new_estimate &&
           (self.loss_based_result.state == LossBasedState::kIncreasing ||
            self.loss_based_result.state ==
                LossBasedState::kIncreaseUsingPadding))) &&
         IsInLossLimitedState();
}

// Returns a `LossBasedBweV2::Config` iff the `key_value_config` specifies a
// configuration for the `LossBasedBweV2` which is explicitly enabled.
Option<LossBasedBweV2::Config> CreateConfig(&self /* LossBasedBweV2 */,
    const FieldTrialsView* key_value_config) {
  FieldTrialParameter<bool> enabled("Enabled", true);
  FieldTrialParameter<f64> bandwidth_rampup_upper_bound_factor(
      "BwRampupUpperBoundFactor", 1000000.0);
  FieldTrialParameter<f64> bandwidth_rampup_upper_bound_factor_in_hold(
      "BwRampupUpperBoundInHoldFactor", 1000000.0);
  FieldTrialParameter<f64> bandwidth_rampup_hold_threshold(
      "BwRampupUpperBoundHoldThreshold", 1.3);
  FieldTrialParameter<f64> rampup_acceleration_max_factor(
      "BwRampupAccelMaxFactor", 0.0);
  FieldTrialParameter<TimeDelta> rampup_acceleration_maxout_time(
      "BwRampupAccelMaxoutTime", Duration::from_secs(60));
  FieldTrialList<f64> candidate_factors("CandidateFactors",
                                           {1.02, 1.0, 0.95});
  FieldTrialParameter<f64> higher_bandwidth_bias_factor("HigherBwBiasFactor",
                                                           0.0002);
  FieldTrialParameter<f64> higher_log_bandwidth_bias_factor(
      "HigherLogBwBiasFactor", 0.02);
  FieldTrialParameter<f64> inherent_loss_lower_bound(
      "InherentLossLowerBound", 1.0e-3);
  FieldTrialParameter<f64> loss_threshold_of_high_bandwidth_preference(
      "LossThresholdOfHighBandwidthPreference", 0.15);
  FieldTrialParameter<f64> bandwidth_preference_smoothing_factor(
      "BandwidthPreferenceSmoothingFactor", 0.002);
  FieldTrialParameter<DataRate> inherent_loss_upper_bound_bandwidth_balance(
      "InherentLossUpperBoundBwBalance", DataRate::KilobitsPerSec(75.0));
  FieldTrialParameter<f64> inherent_loss_upper_bound_offset(
      "InherentLossUpperBoundOffset", 0.05);
  FieldTrialParameter<f64> initial_inherent_loss_estimate(
      "InitialInherentLossEstimate", 0.01);
  FieldTrialParameter<int> newton_iterations("NewtonIterations", 1);
  FieldTrialParameter<f64> newton_step_size("NewtonStepSize", 0.75);
  FieldTrialParameter<bool> append_acknowledged_rate_candidate(
      "AckedRateCandidate", true);
  FieldTrialParameter<bool> append_delay_based_estimate_candidate(
      "DelayBasedCandidate", true);
  FieldTrialParameter<bool> append_upper_bound_candidate_in_alr(
      "UpperBoundCandidateInAlr", false);
  FieldTrialParameter<TimeDelta> observation_duration_lower_bound(
      "ObservationDurationLowerBound", TimeDelta::Millis(250));
  FieldTrialParameter<int> observation_window_size("ObservationWindowSize", 20);
  FieldTrialParameter<f64> sending_rate_smoothing_factor(
      "SendingRateSmoothingFactor", 0.0);
  FieldTrialParameter<f64> instant_upper_bound_temporal_weight_factor(
      "InstantUpperBoundTemporalWeightFactor", 0.9);
  FieldTrialParameter<DataRate> instant_upper_bound_bandwidth_balance(
      "InstantUpperBoundBwBalance", DataRate::KilobitsPerSec(75.0));
  FieldTrialParameter<f64> instant_upper_bound_loss_offset(
      "InstantUpperBoundLossOffset", 0.05);
  FieldTrialParameter<f64> temporal_weight_factor("TemporalWeightFactor",
                                                     0.9);
  FieldTrialParameter<f64> bandwidth_backoff_lower_bound_factor(
      "BwBackoffLowerBoundFactor", 1.0);
  FieldTrialParameter<f64> max_increase_factor("MaxIncreaseFactor", 1.3);
  FieldTrialParameter<TimeDelta> delayed_increase_window(
      "DelayedIncreaseWindow", TimeDelta::Millis(300));
  FieldTrialParameter<bool>
      not_increase_if_inherent_loss_less_than_average_loss(
          "NotIncreaseIfInherentLossLessThanAverageLoss", true);
  FieldTrialParameter<bool> not_use_acked_rate_in_alr("NotUseAckedRateInAlr",
                                                      true);
  FieldTrialParameter<bool> use_in_start_phase("UseInStartPhase", false);
  FieldTrialParameter<int> min_num_observations("MinNumObservations", 3);
  FieldTrialParameter<f64> lower_bound_by_acked_rate_factor(
      "LowerBoundByAckedRateFactor", 0.0);
  FieldTrialParameter<f64> hold_duration_factor("HoldDurationFactor", 0.0);
  FieldTrialParameter<bool> use_byte_loss_rate("UseByteLossRate", false);
  FieldTrialParameter<TimeDelta> padding_duration("PaddingDuration",
                                                  TimeDelta::Zero());
  FieldTrialParameter<bool> bound_best_candidate("BoundBestCandidate", false);
  FieldTrialParameter<bool> pace_at_loss_based_estimate(
      "PaceAtLossBasedEstimate", false);
  FieldTrialParameter<f64> median_sending_rate_factor(
      "MedianSendingRateFactor", 2.0);
  if (key_value_config) {
    ParseFieldTrial({&enabled,
                     &bandwidth_rampup_upper_bound_factor,
                     &bandwidth_rampup_upper_bound_factor_in_hold,
                     &bandwidth_rampup_hold_threshold,
                     &rampup_acceleration_max_factor,
                     &rampup_acceleration_maxout_time,
                     &candidate_factors,
                     &higher_bandwidth_bias_factor,
                     &higher_log_bandwidth_bias_factor,
                     &inherent_loss_lower_bound,
                     &loss_threshold_of_high_bandwidth_preference,
                     &bandwidth_preference_smoothing_factor,
                     &inherent_loss_upper_bound_bandwidth_balance,
                     &inherent_loss_upper_bound_offset,
                     &initial_inherent_loss_estimate,
                     &newton_iterations,
                     &newton_step_size,
                     &append_acknowledged_rate_candidate,
                     &append_delay_based_estimate_candidate,
                     &append_upper_bound_candidate_in_alr,
                     &observation_duration_lower_bound,
                     &observation_window_size,
                     &sending_rate_smoothing_factor,
                     &instant_upper_bound_temporal_weight_factor,
                     &instant_upper_bound_bandwidth_balance,
                     &instant_upper_bound_loss_offset,
                     &temporal_weight_factor,
                     &bandwidth_backoff_lower_bound_factor,
                     &max_increase_factor,
                     &delayed_increase_window,
                     &not_increase_if_inherent_loss_less_than_average_loss,
                     &not_use_acked_rate_in_alr,
                     &use_in_start_phase,
                     &min_num_observations,
                     &lower_bound_by_acked_rate_factor,
                     &hold_duration_factor,
                     &use_byte_loss_rate,
                     &padding_duration,
                     &bound_best_candidate,
                     &pace_at_loss_based_estimate,
                     &median_sending_rate_factor},
                    key_value_config->Lookup("WebRTC-Bwe-LossBasedBweV2"));
  }

  if (!enabled.Get()) {
    return None;
  }
  Config config;
  config.bandwidth_rampup_upper_bound_factor =
      bandwidth_rampup_upper_bound_factor.Get();
  config.bandwidth_rampup_upper_bound_factor_in_hold =
      bandwidth_rampup_upper_bound_factor_in_hold.Get();
  config.bandwidth_rampup_hold_threshold =
      bandwidth_rampup_hold_threshold.Get();
  config.rampup_acceleration_max_factor = rampup_acceleration_max_factor.Get();
  config.rampup_acceleration_maxout_time =
      rampup_acceleration_maxout_time.Get();
  config.candidate_factors = candidate_factors.Get();
  config.higher_bandwidth_bias_factor = higher_bandwidth_bias_factor.Get();
  config.higher_log_bandwidth_bias_factor =
      higher_log_bandwidth_bias_factor.Get();
  config.inherent_loss_lower_bound = inherent_loss_lower_bound.Get();
  config.loss_threshold_of_high_bandwidth_preference =
      loss_threshold_of_high_bandwidth_preference.Get();
  config.bandwidth_preference_smoothing_factor =
      bandwidth_preference_smoothing_factor.Get();
  config.inherent_loss_upper_bound_bandwidth_balance =
      inherent_loss_upper_bound_bandwidth_balance.Get();
  config.inherent_loss_upper_bound_offset =
      inherent_loss_upper_bound_offset.Get();
  config.initial_inherent_loss_estimate = initial_inherent_loss_estimate.Get();
  config.newton_iterations = newton_iterations.Get();
  config.newton_step_size = newton_step_size.Get();
  config.append_acknowledged_rate_candidate =
      append_acknowledged_rate_candidate.Get();
  config.append_delay_based_estimate_candidate =
      append_delay_based_estimate_candidate.Get();
  config.append_upper_bound_candidate_in_alr =
      append_upper_bound_candidate_in_alr.Get();
  config.observation_duration_lower_bound =
      observation_duration_lower_bound.Get();
  config.observation_window_size = observation_window_size.Get();
  config.sending_rate_smoothing_factor = sending_rate_smoothing_factor.Get();
  config.instant_upper_bound_temporal_weight_factor =
      instant_upper_bound_temporal_weight_factor.Get();
  config.instant_upper_bound_bandwidth_balance =
      instant_upper_bound_bandwidth_balance.Get();
  config.instant_upper_bound_loss_offset =
      instant_upper_bound_loss_offset.Get();
  config.temporal_weight_factor = temporal_weight_factor.Get();
  config.bandwidth_backoff_lower_bound_factor =
      bandwidth_backoff_lower_bound_factor.Get();
  config.max_increase_factor = max_increase_factor.Get();
  config.delayed_increase_window = delayed_increase_window.Get();
  config.not_increase_if_inherent_loss_less_than_average_loss =
      not_increase_if_inherent_loss_less_than_average_loss.Get();
  config.not_use_acked_rate_in_alr = not_use_acked_rate_in_alr.Get();
  config.use_in_start_phase = use_in_start_phase.Get();
  config.min_num_observations = min_num_observations.Get();
  config.lower_bound_by_acked_rate_factor =
      lower_bound_by_acked_rate_factor.Get();
  config.hold_duration_factor = hold_duration_factor.Get();
  config.use_byte_loss_rate = use_byte_loss_rate.Get();
  config.padding_duration = padding_duration.Get();
  config.bound_best_candidate = bound_best_candidate.Get();
  config.pace_at_loss_based_estimate = pace_at_loss_based_estimate.Get();
  config.median_sending_rate_factor = median_sending_rate_factor.Get();
  return config;
}

bool IsConfigValid(&self /* LossBasedBweV2 */) {
  if (!self.config.is_some()) {
    return false;
  }

  bool valid = true;

  if (self.config.bandwidth_rampup_upper_bound_factor <= 1.0) {
    RTC_LOG(LS_WARNING)
        << "The bandwidth rampup upper bound factor must be greater than 1: "
        << self.config.bandwidth_rampup_upper_bound_factor;
    valid = false;
  }
  if (self.config.bandwidth_rampup_upper_bound_factor_in_hold <= 1.0) {
    RTC_LOG(LS_WARNING) << "The bandwidth rampup upper bound factor in hold "
                           "must be greater than 1: "
                        << self.config.bandwidth_rampup_upper_bound_factor_in_hold;
    valid = false;
  }
  if (self.config.bandwidth_rampup_hold_threshold < 0.0) {
    RTC_LOG(LS_WARNING) << "The bandwidth rampup hold threshold must"
                           "must be non-negative.: "
                        << self.config.bandwidth_rampup_hold_threshold;
    valid = false;
  }
  if (self.config.rampup_acceleration_max_factor < 0.0) {
    RTC_LOG(LS_WARNING)
        << "The rampup acceleration max factor must be non-negative.: "
        << self.config.rampup_acceleration_max_factor;
    valid = false;
  }
  if (self.config.rampup_acceleration_maxout_time <= TimeDelta::Zero()) {
    RTC_LOG(LS_WARNING)
        << "The rampup acceleration maxout time must be above zero: "
        << self.config.rampup_acceleration_maxout_time.seconds();
    valid = false;
  }
  for (f64 candidate_factor : self.config.candidate_factors) {
    if (candidate_factor <= 0.0) {
      RTC_LOG(LS_WARNING) << "All candidate factors must be greater than zero: "
                          << candidate_factor;
      valid = false;
    }
  }

  // Ensure that the configuration allows generation of at least one candidate
  // other than the current estimate.
  if (!self.config.append_acknowledged_rate_candidate &&
      !self.config.append_delay_based_estimate_candidate &&
      !absl::c_any_of(self.config.candidate_factors,
                      [](f64 cf) { return cf != 1.0; })) {
    RTC_LOG(LS_WARNING)
        << "The configuration does not allow generating candidates. Specify "
           "a candidate factor other than 1.0, allow the acknowledged rate "
           "to be a candidate, and/or allow the delay based estimate to be a "
           "candidate.";
    valid = false;
  }

  if (self.config.higher_bandwidth_bias_factor < 0.0) {
    RTC_LOG(LS_WARNING)
        << "The higher bandwidth bias factor must be non-negative: "
        << self.config.higher_bandwidth_bias_factor;
    valid = false;
  }
  if (self.config.inherent_loss_lower_bound < 0.0 ||
      self.config.inherent_loss_lower_bound >= 1.0) {
    RTC_LOG(LS_WARNING) << "The inherent loss lower bound must be in [0, 1): "
                        << self.config.inherent_loss_lower_bound;
    valid = false;
  }
  if (self.config.loss_threshold_of_high_bandwidth_preference < 0.0 ||
      self.config.loss_threshold_of_high_bandwidth_preference >= 1.0) {
    RTC_LOG(LS_WARNING)
        << "The loss threshold of high bandwidth preference must be in [0, 1): "
        << self.config.loss_threshold_of_high_bandwidth_preference;
    valid = false;
  }
  if (self.config.bandwidth_preference_smoothing_factor <= 0.0 ||
      self.config.bandwidth_preference_smoothing_factor > 1.0) {
    RTC_LOG(LS_WARNING)
        << "The bandwidth preference smoothing factor must be in (0, 1]: "
        << self.config.bandwidth_preference_smoothing_factor;
    valid = false;
  }
  if (self.config.inherent_loss_upper_bound_bandwidth_balance <=
      DataRate::Zero()) {
    RTC_LOG(LS_WARNING)
        << "The inherent loss upper bound bandwidth balance "
           "must be positive: "
        << ToString(self.config.inherent_loss_upper_bound_bandwidth_balance);
    valid = false;
  }
  if (self.config.inherent_loss_upper_bound_offset <
          self.config.inherent_loss_lower_bound ||
      self.config.inherent_loss_upper_bound_offset >= 1.0) {
    RTC_LOG(LS_WARNING) << "The inherent loss upper bound must be greater "
                           "than or equal to the inherent "
                           "loss lower bound, which is "
                        << self.config.inherent_loss_lower_bound
                        << ", and less than 1: "
                        << self.config.inherent_loss_upper_bound_offset;
    valid = false;
  }
  if (self.config.initial_inherent_loss_estimate < 0.0 ||
      self.config.initial_inherent_loss_estimate >= 1.0) {
    RTC_LOG(LS_WARNING)
        << "The initial inherent loss estimate must be in [0, 1): "
        << self.config.initial_inherent_loss_estimate;
    valid = false;
  }
  if (self.config.newton_iterations <= 0) {
    RTC_LOG(LS_WARNING) << "The number of Newton iterations must be positive: "
                        << self.config.newton_iterations;
    valid = false;
  }
  if (self.config.newton_step_size <= 0.0) {
    RTC_LOG(LS_WARNING) << "The Newton step size must be positive: "
                        << self.config.newton_step_size;
    valid = false;
  }
  if (self.config.observation_duration_lower_bound <= TimeDelta::Zero()) {
    RTC_LOG(LS_WARNING)
        << "The observation duration lower bound must be positive: "
        << ToString(self.config.observation_duration_lower_bound);
    valid = false;
  }
  if (self.config.observation_window_size < 2) {
    RTC_LOG(LS_WARNING) << "The observation window size must be at least 2: "
                        << self.config.observation_window_size;
    valid = false;
  }
  if (self.config.sending_rate_smoothing_factor < 0.0 ||
      self.config.sending_rate_smoothing_factor >= 1.0) {
    RTC_LOG(LS_WARNING)
        << "The sending rate smoothing factor must be in [0, 1): "
        << self.config.sending_rate_smoothing_factor;
    valid = false;
  }
  if (self.config.instant_upper_bound_temporal_weight_factor <= 0.0 ||
      self.config.instant_upper_bound_temporal_weight_factor > 1.0) {
    RTC_LOG(LS_WARNING)
        << "The instant upper bound temporal weight factor must be in (0, 1]"
        << self.config.instant_upper_bound_temporal_weight_factor;
    valid = false;
  }
  if (self.config.instant_upper_bound_bandwidth_balance <= DataRate::Zero()) {
    RTC_LOG(LS_WARNING)
        << "The instant upper bound bandwidth balance must be positive: "
        << ToString(self.config.instant_upper_bound_bandwidth_balance);
    valid = false;
  }
  if (self.config.instant_upper_bound_loss_offset < 0.0 ||
      self.config.instant_upper_bound_loss_offset >= 1.0) {
    RTC_LOG(LS_WARNING)
        << "The instant upper bound loss offset must be in [0, 1): "
        << self.config.instant_upper_bound_loss_offset;
    valid = false;
  }
  if (self.config.temporal_weight_factor <= 0.0 ||
      self.config.temporal_weight_factor > 1.0) {
    RTC_LOG(LS_WARNING) << "The temporal weight factor must be in (0, 1]: "
                        << self.config.temporal_weight_factor;
    valid = false;
  }
  if (self.config.bandwidth_backoff_lower_bound_factor > 1.0) {
    RTC_LOG(LS_WARNING)
        << "The bandwidth backoff lower bound factor must not be greater than "
           "1: "
        << self.config.bandwidth_backoff_lower_bound_factor;
    valid = false;
  }
  if (self.config.max_increase_factor <= 0.0) {
    RTC_LOG(LS_WARNING) << "The maximum increase factor must be positive: "
                        << self.config.max_increase_factor;
    valid = false;
  }
  if (self.config.delayed_increase_window <= TimeDelta::Zero()) {
    RTC_LOG(LS_WARNING) << "The delayed increase window must be positive: "
                        << self.config.delayed_increase_window.ms();
    valid = false;
  }
  if (self.config.min_num_observations <= 0) {
    RTC_LOG(LS_WARNING) << "The min number of observations must be positive: "
                        << self.config.min_num_observations;
    valid = false;
  }
  if (self.config.lower_bound_by_acked_rate_factor < 0.0) {
    RTC_LOG(LS_WARNING)
        << "The estimate lower bound by acknowledged rate factor must be "
           "non-negative: "
        << self.config.lower_bound_by_acked_rate_factor;
    valid = false;
  }
  return valid;
}

fn UpdateAverageReportedLossRatio(&self /* LossBasedBweV2 */) {
  self.average_reported_loss_ratio =
      (self.config.use_byte_loss_rate ? CalculateAverageReportedByteLossRatio()
                                   : CalculateAverageReportedPacketLossRatio());
}

f64 CalculateAverageReportedPacketLossRatio(&self /* LossBasedBweV2 */) {
  if (self.num_observations <= 0) {
    return 0.0;
  }

  let num_packets: f64 = 0.0;
  let num_lost_packets: f64 = 0.0;
  for (const Observation& observation : self.observations) {
    if (!observation.IsInitialized()) {
      continue;
    }

    f64 instant_temporal_weight =
        self.instant_upper_bound_temporal_weights[(num_observations_ - 1) -
                                              observation.id];
    num_packets += instant_temporal_weight * observation.num_packets;
    num_lost_packets += instant_temporal_weight * observation.num_lost_packets;
  }

  return num_lost_packets / num_packets;
}

f64 CalculateAverageReportedByteLossRatio(&self /* LossBasedBweV2 */) {
  if (self.num_observations <= 0) {
    return 0.0;
  }

  DataSize total_bytes = DataSize::Zero();
  DataSize lost_bytes = DataSize::Zero();
  let min_loss_rate: f64 = 1.0;
  let max_loss_rate: f64 = 0.0;
  DataSize min_lost_bytes = DataSize::Zero();
  DataSize max_lost_bytes = DataSize::Zero();
  DataSize min_bytes_received = DataSize::Zero();
  DataSize max_bytes_received = DataSize::Zero();
  DataRate send_rate_of_max_loss_observation = DataRate::Zero();
  for (const Observation& observation : self.observations) {
    if (!observation.IsInitialized()) {
      continue;
    }

    f64 instant_temporal_weight =
        self.instant_upper_bound_temporal_weights[(num_observations_ - 1) -
                                              observation.id];
    total_bytes += instant_temporal_weight * observation.size;
    lost_bytes += instant_temporal_weight * observation.lost_size;

    let loss_rate: f64 = !observation.size.IsZero()
                           ? observation.lost_size / observation.size
                           : 0.0;
    if (self.num_observations > 3) {
      if (loss_rate > max_loss_rate) {
        max_loss_rate = loss_rate;
        max_lost_bytes = instant_temporal_weight * observation.lost_size;
        max_bytes_received = instant_temporal_weight * observation.size;
        send_rate_of_max_loss_observation = observation.sending_rate;
      }
      if (loss_rate < min_loss_rate) {
        min_loss_rate = loss_rate;
        min_lost_bytes = instant_temporal_weight * observation.lost_size;
        min_bytes_received = instant_temporal_weight * observation.size;
      }
    }
  }
  if (GetMedianSendingRate() * self.config.median_sending_rate_factor <=
      send_rate_of_max_loss_observation) {
    // If the median sending rate is less than half of the sending rate of the
    // observation with max loss rate, i.e. we suddenly send a lot of data, then
    // the loss rate might not be due to a spike.
    return lost_bytes / total_bytes;
  }
  return (lost_bytes - min_lost_bytes - max_lost_bytes) /
         (total_bytes - max_bytes_received - min_bytes_received);
}

DataRate GetCandidateBandwidthUpperBound(&self /* LossBasedBweV2 */) {
  DataRate candidate_bandwidth_upper_bound = self.max_bitrate;
  if (IsInLossLimitedState() && IsValid(self.bandwidth_limit_in_current_window)) {
    candidate_bandwidth_upper_bound = self.bandwidth_limit_in_current_window;
  }

  if (!self.acknowledged_bitrate.is_some())
    return candidate_bandwidth_upper_bound;

  if (self.config.rampup_acceleration_max_factor > 0.0) {
    const TimeDelta time_since_bandwidth_reduced = std::cmp::min(
        self.config.rampup_acceleration_maxout_time,
        std::cmp::max(TimeDelta::Zero(), last_send_time_most_recent_observation_ -
                                        self.last_time_estimate_reduced));
    let rampup_acceleration: f64 = self.config.rampup_acceleration_max_factor *
                                       time_since_bandwidth_reduced /
                                       self.config.rampup_acceleration_maxout_time;

    candidate_bandwidth_upper_bound +=
        rampup_acceleration * (*self.acknowledged_bitrate);
  }
  return candidate_bandwidth_upper_bound;
}

Vec<LossBasedBweV2::ChannelParameters> GetCandidates(&self /* LossBasedBweV2 */,
    bool in_alr) {
  ChannelParameters best_estimate = self.current_best_estimate;
  Vec<DataRate> bandwidths;
  for (f64 candidate_factor : self.config.candidate_factors) {
    bandwidths.push_back(candidate_factor *
                         best_estimate.loss_limited_bandwidth);
  }

  if (self.acknowledged_bitrate.is_some() &&
      self.config.append_acknowledged_rate_candidate) {
    if (!(self.config.not_use_acked_rate_in_alr && in_alr) ||
        (self.config.padding_duration > TimeDelta::Zero() &&
         self.last_padding_info.padding_timestamp + self.config.padding_duration >=
             self.last_send_time_most_recent_observation)) {
      bandwidths.push_back(*self.acknowledged_bitrate *
                           self.config.bandwidth_backoff_lower_bound_factor);
    }
  }

  if (IsValid(self.delay_based_estimate) &&
      self.config.append_delay_based_estimate_candidate) {
    if (self.delay_based_estimate > best_estimate.loss_limited_bandwidth) {
      bandwidths.push_back(self.delay_based_estimate);
    }
  }

  if (in_alr && self.config.append_upper_bound_candidate_in_alr &&
      best_estimate.loss_limited_bandwidth > GetInstantUpperBound()) {
    bandwidths.push_back(GetInstantUpperBound());
  }

  const DataRate candidate_bandwidth_upper_bound =
      GetCandidateBandwidthUpperBound();

  Vec<ChannelParameters> candidates;
  candidates.resize(bandwidths.len());
  for i = 0; i < bandwidths.len()i += 1) {
    ChannelParameters candidate = best_estimate;
    candidate.loss_limited_bandwidth =
        std::cmp::min(bandwidths[i], std::cmp::max(best_estimate.loss_limited_bandwidth,
                                         candidate_bandwidth_upper_bound));
    candidate.inherent_loss = GetFeasibleInherentLoss(candidate);
    candidates[i] = candidate;
  }
  return candidates;
}

LossBasedBweV2::Derivatives GetDerivatives(&self /* LossBasedBweV2 */,
    const ChannelParameters& channel_parameters) {
  Derivatives derivatives;

  for (const Observation& observation : self.observations) {
    if (!observation.IsInitialized()) {
      continue;
    }

    let loss_probability: f64 = GetLossProbability(
        channel_parameters.inherent_loss,
        channel_parameters.loss_limited_bandwidth, observation.sending_rate);

    f64 temporal_weight =
        self.temporal_weights[(num_observations_ - 1) - observation.id];
    if (self.config.use_byte_loss_rate) {
      derivatives.first +=
          temporal_weight *
          ((ToKiloBytes(observation.lost_size) / loss_probability) -
           (ToKiloBytes(observation.size - observation.lost_size) /
            (1.0 - loss_probability)));
      derivatives.second -=
          temporal_weight *
          ((ToKiloBytes(observation.lost_size) /
            std::pow(loss_probability, 2)) +
           (ToKiloBytes(observation.size - observation.lost_size) /
            std::pow(1.0 - loss_probability, 2)));
    } else {
      derivatives.first +=
          temporal_weight *
          ((observation.num_lost_packets / loss_probability) -
           (observation.num_received_packets / (1.0 - loss_probability)));
      derivatives.second -=
          temporal_weight *
          ((observation.num_lost_packets / std::pow(loss_probability, 2)) +
           (observation.num_received_packets /
            std::pow(1.0 - loss_probability, 2)));
    }
  }

  if (derivatives.second >= 0.0) {
    RTC_LOG(LS_ERROR) << "The second derivative is mathematically guaranteed "
                         "to be negative but is "
                      << derivatives.second << ".";
    derivatives.second = -1.0e-6;
  }

  return derivatives;
}

f64 GetFeasibleInherentLoss(&self /* LossBasedBweV2 */,
    const ChannelParameters& channel_parameters) {
  return std::cmp::min(
      std::cmp::max(channel_parameters.inherent_loss,
               self.config.inherent_loss_lower_bound),
      GetInherentLossUpperBound(channel_parameters.loss_limited_bandwidth));
}

f64 GetInherentLossUpperBound(&self /* LossBasedBweV2 */,DataRate bandwidth) {
  if (bandwidth.IsZero()) {
    return 1.0;
  }

  f64 inherent_loss_upper_bound =
      self.config.inherent_loss_upper_bound_offset +
      self.config.inherent_loss_upper_bound_bandwidth_balance / bandwidth;
  return std::cmp::min(inherent_loss_upper_bound, 1.0);
}

f64 AdjustBiasFactor(&self /* LossBasedBweV2 */,f64 loss_rate,
                                        f64 bias_factor) {
  return bias_factor *
         (self.config.loss_threshold_of_high_bandwidth_preference - loss_rate) /
         (self.config.bandwidth_preference_smoothing_factor +
          std::abs(self.config.loss_threshold_of_high_bandwidth_preference -
                   loss_rate));
}

f64 GetHighBandwidthBias(&self /* LossBasedBweV2 */,DataRate bandwidth) {
  if (IsValid(bandwidth)) {
    return AdjustBiasFactor(self.average_reported_loss_ratio,
                            self.config.higher_bandwidth_bias_factor) *
               bandwidth.kbps() +
           AdjustBiasFactor(self.average_reported_loss_ratio,
                            self.config.higher_log_bandwidth_bias_factor) *
               std::log(1.0 + bandwidth.kbps());
  }
  return 0.0;
}

f64 GetObjective(&self /* LossBasedBweV2 */,
    const ChannelParameters& channel_parameters) {
  let objective: f64 = 0.0;

  f64 high_bandwidth_bias =
      GetHighBandwidthBias(channel_parameters.loss_limited_bandwidth);

  for (const Observation& observation : self.observations) {
    if (!observation.IsInitialized()) {
      continue;
    }

    let loss_probability: f64 = GetLossProbability(
        channel_parameters.inherent_loss,
        channel_parameters.loss_limited_bandwidth, observation.sending_rate);

    f64 temporal_weight =
        self.temporal_weights[(num_observations_ - 1) - observation.id];
    if (self.config.use_byte_loss_rate) {
      objective +=
          temporal_weight *
          ((ToKiloBytes(observation.lost_size) * std::log(loss_probability)) +
           (ToKiloBytes(observation.size - observation.lost_size) *
            std::log(1.0 - loss_probability)));
      objective +=
          temporal_weight * high_bandwidth_bias * ToKiloBytes(observation.size);
    } else {
      objective +=
          temporal_weight *
          ((observation.num_lost_packets * std::log(loss_probability)) +
           (observation.num_received_packets *
            std::log(1.0 - loss_probability)));
      objective +=
          temporal_weight * high_bandwidth_bias * observation.num_packets;
    }
  }

  return objective;
}

DataRate GetSendingRate(&self /* LossBasedBweV2 */,
    DataRate instantaneous_sending_rate) {
  if (self.num_observations <= 0) {
    return instantaneous_sending_rate;
  }

  const int most_recent_observation_idx =
      (num_observations_ - 1) % self.config.observation_window_size;
  const Observation& most_recent_observation =
      self.observations[most_recent_observation_idx];
  DataRate sending_rate_previous_observation =
      most_recent_observation.sending_rate;

  return self.config.sending_rate_smoothing_factor *
             sending_rate_previous_observation +
         (1.0 - self.config.sending_rate_smoothing_factor) *
             instantaneous_sending_rate;
}

DataRate GetInstantUpperBound(&self /* LossBasedBweV2 */) {
  return self.cached_instant_upper_bound.unwrap_or(self.max_bitrate);
}

fn CalculateInstantUpperBound(&self /* LossBasedBweV2 */) {
  DataRate instant_limit = self.max_bitrate;
  if (self.average_reported_loss_ratio > self.config.instant_upper_bound_loss_offset) {
    instant_limit = self.config.instant_upper_bound_bandwidth_balance /
                    (average_reported_loss_ratio_ -
                     self.config.instant_upper_bound_loss_offset);
  }

  self.cached_instant_upper_bound = instant_limit;
}

DataRate GetInstantLowerBound(&self /* LossBasedBweV2 */) {
  return self.cached_instant_lower_bound.unwrap_or(DataRate::Zero());
}

fn CalculateInstantLowerBound(&self /* LossBasedBweV2 */) {
  DataRate instance_lower_bound = DataRate::Zero();
  if (IsValid(self.acknowledged_bitrate) &&
      self.config.lower_bound_by_acked_rate_factor > 0.0) {
    instance_lower_bound = self.config.lower_bound_by_acked_rate_factor *
                           self.acknowledged_bitrate.value();
  }

  if (IsValid(self.min_bitrate)) {
    instance_lower_bound = std::cmp::max(instance_lower_bound, self.min_bitrate);
  }
  self.cached_instant_lower_bound = instance_lower_bound;
}

fn CalculateTemporalWeights(&self /* LossBasedBweV2 */) {
  for (int i = 0; i < self.config.observation_window_sizei += 1) {
    self.temporal_weights[i] = std::pow(self.config.temporal_weight_factor, i);
    self.instant_upper_bound_temporal_weights[i] =
        std::pow(self.config.instant_upper_bound_temporal_weight_factor, i);
  }
}

fn NewtonsMethodUpdate(&self /* LossBasedBweV2 */,
    ChannelParameters& channel_parameters) {
  if (self.num_observations <= 0) {
    return;
  }

  for (int i = 0; i < self.config.newton_iterationsi += 1) {
    const Derivatives derivatives = GetDerivatives(channel_parameters);
    channel_parameters.inherent_loss -=
        self.config.newton_step_size * derivatives.first / derivatives.second;
    channel_parameters.inherent_loss =
        GetFeasibleInherentLoss(channel_parameters);
  }
}

bool PushBackObservation(&self /* LossBasedBweV2 */,
    rtc::ArrayView<const PacketResult> packet_results) {
  if (packet_results.empty()) {
    return false;
  }

  self.partial_observation.num_packets += packet_results.len();
  Timestamp last_send_time = Timestamp::MinusInfinity();
  Timestamp first_send_time = Timestamp::PlusInfinity();
  for (const PacketResult& packet : packet_results) {
    if (packet.IsReceived()) {
      self.partial_observation.lost_packets.erase(
          packet.sent_packet.sequence_number);
    } else {
      self.partial_observation.lost_packets.emplace(
          packet.sent_packet.sequence_number, packet.sent_packet.size);
    }
    self.partial_observation.size += packet.sent_packet.size;
    last_send_time = std::cmp::max(last_send_time, packet.sent_packet.send_time);
    first_send_time = std::cmp::min(first_send_time, packet.sent_packet.send_time);
  }

  // This is the first packet report we have received.
  if (!IsValid(self.last_send_time_most_recent_observation)) {
    self.last_send_time_most_recent_observation = first_send_time;
  }

  const TimeDelta observation_duration =
      last_send_time - self.last_send_time_most_recent_observation;
  // Too small to be meaningful.
  // To consider: what if it is too long?, i.e. we did not receive any packets
  // for a long time, then all the packets we received are too old.
  if (observation_duration <= TimeDelta::Zero() ||
      observation_duration < self.config.observation_duration_lower_bound) {
    return false;
  }

  self.last_send_time_most_recent_observation = last_send_time;

  Observation observation;
  observation.num_packets = self.partial_observation.num_packets;
  observation.num_lost_packets = self.partial_observation.lost_packets.len();
  observation.num_received_packets =
      observation.num_packets - observation.num_lost_packets;
  observation.sending_rate =
      GetSendingRate(self.partial_observation.size / observation_duration);
  for (auto const& [key, packet_size] : self.partial_observation.lost_packets) {
    observation.lost_size += packet_size;
  }
  observation.size = self.partial_observation.size;
  observation.id = self.num_observations += 1;
  self.observations[observation.id % self.config.observation_window_size] =
      observation;

  self.partial_observation = PartialObservation();
  UpdateAverageReportedLossRatio();
  CalculateInstantUpperBound();
  return true;
}

bool IsInLossLimitedState(&self /* LossBasedBweV2 */) {
  return self.loss_based_result.state != LossBasedState::kDelayBasedEstimate;
}

bool CanKeepIncreasingState(&self /* LossBasedBweV2 */,DataRate estimate) {
  if (self.config.padding_duration == TimeDelta::Zero() ||
      self.loss_based_result.state != LossBasedState::kIncreaseUsingPadding)
    return true;

  // Keep using the kIncreaseUsingPadding if either the state has been
  // kIncreaseUsingPadding for less than kPaddingDuration or the estimate
  // increases.
  return self.last_padding_info.padding_timestamp + self.config.padding_duration >=
             last_send_time_most_recent_observation_ ||
         self.last_padding_info.padding_rate < estimate;
}

bool PaceAtLossBasedEstimate(&self /* LossBasedBweV2 */) {
  return self.config.pace_at_loss_based_estimate &&
         self.loss_based_result.state != LossBasedState::kDelayBasedEstimate;
}

DataRate GetMedianSendingRate(&self /* LossBasedBweV2 */) {
  Vec<DataRate> sending_rates;
  for (const Observation& observation : self.observations) {
    if (!observation.IsInitialized() || !IsValid(observation.sending_rate) ||
        observation.sending_rate.IsZero()) {
      continue;
    }
    sending_rates.push_back(observation.sending_rate);
  }
  if (sending_rates.empty()) {
    return DataRate::Zero();
  }
  absl::c_sort(sending_rates);
  if (sending_rates.len() % 2 == 0) {
    return (sending_rates[sending_rates.len() / 2 - 1] +
            sending_rates[sending_rates.len() / 2]) /
           2;
  }
  return sending_rates[sending_rates.len() / 2];
}

}  // namespace webrtc
