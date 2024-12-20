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
  Increasing = 0,
  // TODO(bugs.webrtc.org/12707): Remove one of the increasing states once we
  // have decided if padding is usefull for ramping up when BWE is loss
  // limited.
  IncreaseUsingPadding = 1,
  Decreasing = 2,
  DelayBasedEstimate = 3
};

pub struct LossBasedBweV2 {
 public:
  struct Result {
    ~Result() = default;
    let bandwidth_estimate: DataRate = DataRate::Zero();
    // State is used by goog_cc, which later sends probe requests to probe
    // controller if state is Increasing.
    let state: LossBasedState = LossBasedState::kDelayBasedEstimate;
  };
  // Creates a disabled `LossBasedBweV2` if the
  // `key_value_config` is not valid.
  explicit LossBasedBweV2(const FieldTrialsView* key_value_config);

  LossBasedBweV2(const LossBasedBweV2&) = delete;
  LossBasedBweV2& operator=(const LossBasedBweV2&) = delete;

  ~LossBasedBweV2() = default;

  fn IsEnabled(&self) -> bool;
  // Returns true iff a BWE can be calculated, i.e., the estimator has been
  // initialized with a BWE and then has received enough `PacketResult`s.
  fn IsReady(&self) -> bool;

  // Returns true if loss based BWE is ready to be used in the start phase.
  fn ReadyToUseInStartPhase(&self) -> bool;

  // Returns true if loss based BWE can be used in the start phase.
  fn UseInStartPhase(&self) -> bool;

  // Returns `DataRate::PlusInfinity` if no BWE can be calculated.
  fn GetLossBasedResult(&self) -> Result;

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
  fn PaceAtLossBasedEstimate(&self) -> bool;

  // For unit testing only.
  fn SetBandwidthEstimate(DataRate bandwidth_estimate) {
  todo!();
}

 private:
  struct ChannelParameters {
    let inherent_loss: f64 = 0.0;
    let loss_limited_bandwidth: DataRate = DataRate::MinusInfinity();
  };

  struct Config {
    let bandwidth_rampup_upper_bound_factor: f64 = 0.0;
    let bandwidth_rampup_upper_bound_factor_in_hold: f64 = 0.0;
    let bandwidth_rampup_hold_threshold: f64 = 0.0;
    let rampup_acceleration_max_factor: f64 = 0.0;
    let rampup_acceleration_maxout_time: TimeDelta = TimeDelta::Zero();
    Vec<f64> candidate_factors;
    let higher_bandwidth_bias_factor: f64 = 0.0;
    let higher_log_bandwidth_bias_factor: f64 = 0.0;
    let inherent_loss_lower_bound: f64 = 0.0;
    let loss_threshold_of_high_bandwidth_preference: f64 = 0.0;
    let bandwidth_preference_smoothing_factor: f64 = 0.0;
    let inherent_loss_upper_bound_bandwidth_balance: DataRate =
        DataRate::MinusInfinity();
    let inherent_loss_upper_bound_offset: f64 = 0.0;
    let initial_inherent_loss_estimate: f64 = 0.0;
    let newton_iterations: isize = 0;
    let newton_step_size: f64 = 0.0;
    let append_acknowledged_rate_candidate: bool = true;
    let append_delay_based_estimate_candidate: bool = false;
    let append_upper_bound_candidate_in_alr: bool = false;
    let observation_duration_lower_bound: TimeDelta = TimeDelta::Zero();
    let observation_window_size: isize = 0;
    let sending_rate_smoothing_factor: f64 = 0.0;
    let instant_upper_bound_temporal_weight_factor: f64 = 0.0;
    let instant_upper_bound_bandwidth_balance: DataRate = DataRate::MinusInfinity();
    let instant_upper_bound_loss_offset: f64 = 0.0;
    let temporal_weight_factor: f64 = 0.0;
    let bandwidth_backoff_lower_bound_factor: f64 = 0.0;
    let max_increase_factor: f64 = 0.0;
    let delayed_increase_window: TimeDelta = TimeDelta::Zero();
    let not_increase_if_inherent_loss_less_than_average_loss: bool = false;
    let not_use_acked_rate_in_alr: bool = false;
    let use_in_start_phase: bool = false;
    let min_num_observations: isize = 0;
    let lower_bound_by_acked_rate_factor: f64 = 0.0;
    let hold_duration_factor: f64 = 0.0;
    let use_byte_loss_rate: bool = false;
    let padding_duration: TimeDelta = TimeDelta::Zero();
    let bound_best_candidate: bool = false;
    let pace_at_loss_based_estimate: bool = false;
    let median_sending_rate_factor: f64 = 0.0;
  };

  struct Derivatives {
    let first: f64 = 0.0;
    let second: f64 = 0.0;
  };

  struct Observation {
    bool IsInitialized() { return id != -1; }

    let num_packets: isize = 0;
    let num_lost_packets: isize = 0;
    let num_received_packets: isize = 0;
    let sending_rate: DataRate = DataRate::MinusInfinity();
    let size: DataSize = DataSize::Zero();
    let lost_size: DataSize = DataSize::Zero();
    let id: isize = -1;
  };

  struct PartialObservation {
    let num_packets: isize = 0;
    std::unordered_map<i64, DataSize> lost_packets;
    let size: DataSize = DataSize::Zero();
  };

  struct PaddingInfo {
    let padding_rate: DataRate = DataRate::MinusInfinity();
    let padding_timestamp: Timestamp = Timestamp::MinusInfinity();
  };

  struct HoldInfo {
    let timestamp: Timestamp = Timestamp::MinusInfinity();
    let duration: TimeDelta = TimeDelta::Zero();
    let rate: DataRate = DataRate::PlusInfinity();
  };

  static Option<Config> CreateConfig(
      const FieldTrialsView* key_value_config);
  fn IsConfigValid(&self) -> bool;

  // Returns `0.0` if not enough loss statistics have been received.
  fn UpdateAverageReportedLossRatio(&mut self) { todo!(); }
  fn CalculateAverageReportedPacketLossRatio(&self) -> f64;
  // Calculates the average loss ratio over the last `observation_window_size`
  // observations but skips the observation with min and max loss ratio in order
  // to filter out loss spikes.
  fn CalculateAverageReportedByteLossRatio(&self) -> f64;
  Vec<ChannelParameters> GetCandidates(bool in_alr) const;
  fn GetCandidateBandwidthUpperBound(&self) -> DataRate;
  fn GetDerivatives(&self, ChannelParameters: const& channel_parameters) -> Derivatives;
  f64 GetFeasibleInherentLoss(
      const ChannelParameters& channel_parameters) const;
  fn GetInherentLossUpperBound(&self, bandwidth: DataRate) -> f64;
  fn AdjustBiasFactor(&self, f64: loss_rate, f64: bias_factor) -> f64;
  fn GetHighBandwidthBias(&self, bandwidth: DataRate) -> f64;
  fn GetObjective(&self, ChannelParameters: const& channel_parameters) -> f64;
  fn GetSendingRate(&self, instantaneous_sending_rate: DataRate) -> DataRate;
  fn GetInstantUpperBound(&self) -> DataRate;
  fn CalculateInstantUpperBound(&mut self) { todo!(); }
  fn GetInstantLowerBound(&self) -> DataRate;
  fn CalculateInstantLowerBound(&mut self) { todo!(); }

  fn CalculateTemporalWeights(&mut self) { todo!(); }
  fn NewtonsMethodUpdate(&self, ChannelParameters& channel_parameters) -> void;

  // Returns false if no observation was created.
  bool PushBackObservation(rtc::ArrayView<const PacketResult> packet_results);
  bool IsEstimateIncreasingWhenLossLimited(DataRate old_estimate,
                                           DataRate new_estimate);
  fn IsInLossLimitedState(&self) -> bool;
  fn CanKeepIncreasingState(&self, estimate: DataRate) -> bool;
  fn GetMedianSendingRate(&self) -> DataRate;

  acknowledged_bitrate: Option<DataRate>,
  config: Option<Config>,
  current_best_estimate: ChannelParameters,
  isize self.num_observations = 0;
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
