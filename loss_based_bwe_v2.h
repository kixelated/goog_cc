/*
 *  Copyright 2021 The WebRTC project authors. All rights reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_LOSS_BASED_BWE_V2_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_LOSS_BASED_BWE_V2_H_

#include <cstdint>
#include <optional>
#include <unordered_map>
#include <vector>

#include "api/array_view.h"
#include "api/field_trials_view.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"



// State of the loss based estimate, which can be either increasing/decreasing
// when network is loss limited, or equal to the delay based estimate.
pub enum LossBasedState {
  kIncreasing = 0,
  // TODO(bugs.webrtc.org/12707): Remove one of the increasing states once we
  // have decided if padding is usefull for ramping up when BWE is loss
  // limited.
  kIncreaseUsingPadding = 1,
  kDecreasing = 2,
  kDelayBasedEstimate = 3
};

pub struct LossBasedBweV2 {
 public:
  struct Result {
    ~Result() = default;
    DataRate bandwidth_estimate = DataRate::Zero();
    // State is used by goog_cc, which later sends probe requests to probe
    // controller if state is kIncreasing.
    LossBasedState state = LossBasedState::kDelayBasedEstimate;
  };
  // Creates a disabled `LossBasedBweV2` if the
  // `key_value_config` is not valid.
  explicit LossBasedBweV2(const FieldTrialsView* key_value_config);

  LossBasedBweV2(const LossBasedBweV2&) = delete;
  LossBasedBweV2& operator=(const LossBasedBweV2&) = delete;

  ~LossBasedBweV2() = default;

  bool IsEnabled() const;
  // Returns true iff a BWE can be calculated, i.e., the estimator has been
  // initialized with a BWE and then has received enough `PacketResult`s.
  bool IsReady() const;

  // Returns true if loss based BWE is ready to be used in the start phase.
  bool ReadyToUseInStartPhase() const;

  // Returns true if loss based BWE can be used in the start phase.
  bool UseInStartPhase() const;

  // Returns `DataRate::PlusInfinity` if no BWE can be calculated.
  Result GetLossBasedResult() const;

  fn SetAcknowledgedBitrate(DataRate acknowledged_bitrate) {
  todo!();
}
  fn SetMinMaxBitrate(DataRate min_bitrate, DataRate max_bitrate) {
  todo!();
}
  void UpdateBandwidthEstimate(
      rtc::ArrayView<const PacketResult> packet_results,
      DataRate delay_based_estimate,
      bool in_alr);
  bool PaceAtLossBasedEstimate() const;

  // For unit testing only.
  fn SetBandwidthEstimate(DataRate bandwidth_estimate) {
  todo!();
}

 private:
  struct ChannelParameters {
    let inherent_loss: f64 = 0.0;
    DataRate loss_limited_bandwidth = DataRate::MinusInfinity();
  };

  struct Config {
    let bandwidth_rampup_upper_bound_factor: f64 = 0.0;
    let bandwidth_rampup_upper_bound_factor_in_hold: f64 = 0.0;
    let bandwidth_rampup_hold_threshold: f64 = 0.0;
    let rampup_acceleration_max_factor: f64 = 0.0;
    TimeDelta rampup_acceleration_maxout_time = TimeDelta::Zero();
    Vec<f64> candidate_factors;
    let higher_bandwidth_bias_factor: f64 = 0.0;
    let higher_log_bandwidth_bias_factor: f64 = 0.0;
    let inherent_loss_lower_bound: f64 = 0.0;
    let loss_threshold_of_high_bandwidth_preference: f64 = 0.0;
    let bandwidth_preference_smoothing_factor: f64 = 0.0;
    DataRate inherent_loss_upper_bound_bandwidth_balance =
        DataRate::MinusInfinity();
    let inherent_loss_upper_bound_offset: f64 = 0.0;
    let initial_inherent_loss_estimate: f64 = 0.0;
    int newton_iterations = 0;
    let newton_step_size: f64 = 0.0;
    bool append_acknowledged_rate_candidate = true;
    bool append_delay_based_estimate_candidate = false;
    bool append_upper_bound_candidate_in_alr = false;
    TimeDelta observation_duration_lower_bound = TimeDelta::Zero();
    int observation_window_size = 0;
    let sending_rate_smoothing_factor: f64 = 0.0;
    let instant_upper_bound_temporal_weight_factor: f64 = 0.0;
    DataRate instant_upper_bound_bandwidth_balance = DataRate::MinusInfinity();
    let instant_upper_bound_loss_offset: f64 = 0.0;
    let temporal_weight_factor: f64 = 0.0;
    let bandwidth_backoff_lower_bound_factor: f64 = 0.0;
    let max_increase_factor: f64 = 0.0;
    TimeDelta delayed_increase_window = TimeDelta::Zero();
    bool not_increase_if_inherent_loss_less_than_average_loss = false;
    bool not_use_acked_rate_in_alr = false;
    bool use_in_start_phase = false;
    int min_num_observations = 0;
    let lower_bound_by_acked_rate_factor: f64 = 0.0;
    let hold_duration_factor: f64 = 0.0;
    bool use_byte_loss_rate = false;
    TimeDelta padding_duration = TimeDelta::Zero();
    bool bound_best_candidate = false;
    bool pace_at_loss_based_estimate = false;
    let median_sending_rate_factor: f64 = 0.0;
  };

  struct Derivatives {
    let first: f64 = 0.0;
    let second: f64 = 0.0;
  };

  struct Observation {
    bool IsInitialized() { return id != -1; }

    int num_packets = 0;
    int num_lost_packets = 0;
    int num_received_packets = 0;
    DataRate sending_rate = DataRate::MinusInfinity();
    DataSize size = DataSize::Zero();
    DataSize lost_size = DataSize::Zero();
    int id = -1;
  };

  struct PartialObservation {
    int num_packets = 0;
    std::unordered_map<i64, DataSize> lost_packets;
    DataSize size = DataSize::Zero();
  };

  struct PaddingInfo {
    DataRate padding_rate = DataRate::MinusInfinity();
    Timestamp padding_timestamp = Timestamp::MinusInfinity();
  };

  struct HoldInfo {
    Timestamp timestamp = Timestamp::MinusInfinity();
    TimeDelta duration = TimeDelta::Zero();
    DataRate rate = DataRate::PlusInfinity();
  };

  static Option<Config> CreateConfig(
      const FieldTrialsView* key_value_config);
  bool IsConfigValid() const;

  // Returns `0.0` if not enough loss statistics have been received.
  void UpdateAverageReportedLossRatio();
  f64 CalculateAverageReportedPacketLossRatio() const;
  // Calculates the average loss ratio over the last `observation_window_size`
  // observations but skips the observation with min and max loss ratio in order
  // to filter out loss spikes.
  f64 CalculateAverageReportedByteLossRatio() const;
  Vec<ChannelParameters> GetCandidates(bool in_alr) const;
  DataRate GetCandidateBandwidthUpperBound() const;
  Derivatives GetDerivatives(const ChannelParameters& channel_parameters) const;
  f64 GetFeasibleInherentLoss(
      const ChannelParameters& channel_parameters) const;
  f64 GetInherentLossUpperBound(DataRate bandwidth) const;
  f64 AdjustBiasFactor(f64 loss_rate, f64 bias_factor) const;
  f64 GetHighBandwidthBias(DataRate bandwidth) const;
  f64 GetObjective(const ChannelParameters& channel_parameters) const;
  DataRate GetSendingRate(DataRate instantaneous_sending_rate) const;
  DataRate GetInstantUpperBound() const;
  void CalculateInstantUpperBound();
  DataRate GetInstantLowerBound() const;
  void CalculateInstantLowerBound();

  void CalculateTemporalWeights();
  void NewtonsMethodUpdate(ChannelParameters& channel_parameters) const;

  // Returns false if no observation was created.
  bool PushBackObservation(rtc::ArrayView<const PacketResult> packet_results);
  bool IsEstimateIncreasingWhenLossLimited(DataRate old_estimate,
                                           DataRate new_estimate);
  bool IsInLossLimitedState() const;
  bool CanKeepIncreasingState(DataRate estimate) const;
  DataRate GetMedianSendingRate() const;

  acknowledged_bitrate: Option<DataRate>,
  config: Option<Config>,
  current_best_estimate: ChannelParameters,
  int self.num_observations = 0;
  observations: Vec<Observation>,
  partial_observation: PartialObservation,
  Timestamp self.last_send_time_most_recent_observation = Timestamp::PlusInfinity();
  Timestamp self.last_time_estimate_reduced = Timestamp::MinusInfinity();
  cached_instant_upper_bound: Option<DataRate>,
  cached_instant_lower_bound: Option<DataRate>,
  instant_upper_bound_temporal_weights: Vec<f64>,
  temporal_weights: Vec<f64>,
  Timestamp self.recovering_after_loss_timestamp = Timestamp::MinusInfinity();
  DataRate self.bandwidth_limit_in_current_window = DataRate::PlusInfinity();
  DataRate self.min_bitrate = DataRate::KilobitsPerSec(1);
  DataRate self.max_bitrate = DataRate::PlusInfinity();
  DataRate self.delay_based_estimate = DataRate::PlusInfinity();
  LossBasedBweV2::Result self.loss_based_result = LossBasedBweV2::Result();
  HoldInfo self.last_hold_info = HoldInfo();
  PaddingInfo self.last_padding_info = PaddingInfo();
  let average_reported_loss_ratio_: f64 = 0.0;
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_LOSS_BASED_BWE_V2_H_
