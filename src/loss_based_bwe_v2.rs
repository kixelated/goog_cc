/*
 *  Copyright 2021 The WebRTC project authors. All rights reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::HashMap;

use crate::api::{
    transport::PacketResult,
    units::{DataRate, DataSize, TimeDelta, Timestamp},
};

// State of the loss based estimate, which can be either increasing/decreasing
// when network is loss limited, or equal to the delay based estimate.
pub enum LossBasedState {
    Increasing = 0,
    // TODO(bugs.webrtc.org/12707): Remove one of the increasing states once we
    // have decided if padding is usefull for ramping up when BWE is loss
    // limited.
    IncreaseUsingPadding = 1,
    Decreasing = 2,
    DelayBasedEstimate = 3,
}
struct Result {
    pub bandwidth_estimate: DataRate,
    // State is used by goog_cc, which later sends probe requests to probe
    // controller if state is Increasing.
    pub state: LossBasedState,
}

impl Default for Result {
    fn default() -> Self {
        Self {
            bandwidth_estimate: DataRate::Zero(),
            state: LossBasedState::DelayBasedEstimate,
        }
    }
}

struct ChannelParameters {
    inherent_loss: f64,
    loss_limited_bandwidth: DataRate,
}

impl Default for ChannelParameters {
    fn default() -> Self {
        Self {
            inherent_loss: 0.0,
            loss_limited_bandwidth: DataRate::MinusInfinity(),
        }
    }
}

struct Config {
    pub bandwidth_rampup_upper_bound_factor: f64,
    pub bandwidth_rampup_upper_bound_factor_in_hold: f64,
    pub bandwidth_rampup_hold_threshold: f64,
    pub rampup_acceleration_max_factor: f64,
    pub rampup_acceleration_maxout_time: TimeDelta,
    pub candidate_factors: Vec<f64>,
    pub higher_bandwidth_bias_factor: f64,
    pub higher_log_bandwidth_bias_factor: f64,
    pub inherent_loss_lower_bound: f64,
    pub loss_threshold_of_high_bandwidth_preference: f64,
    pub bandwidth_preference_smoothing_factor: f64,
    pub inherent_loss_upper_bound_bandwidth_balance: DataRate,
    pub inherent_loss_upper_bound_offset: f64,
    pub initial_inherent_loss_estimate: f64,
    pub newton_iterations: isize,
    pub newton_step_size: f64,
    pub append_acknowledged_rate_candidate: bool,
    pub append_delay_based_estimate_candidate: bool,
    pub append_upper_bound_candidate_in_alr: bool,
    pub observation_duration_lower_bound: TimeDelta,
    pub observation_window_size: isize,
    pub sending_rate_smoothing_factor: f64,
    pub instant_upper_bound_temporal_weight_factor: f64,
    pub instant_upper_bound_bandwidth_balance: DataRate,
    pub instant_upper_bound_loss_offset: f64,
    pub temporal_weight_factor: f64,
    pub bandwidth_backoff_lower_bound_factor: f64,
    pub max_increase_factor: f64,
    pub delayed_increase_window: TimeDelta,
    pub not_increase_if_inherent_loss_less_than_average_loss: bool,
    pub not_use_acked_rate_in_alr: bool,
    pub use_in_start_phase: bool,
    pub min_num_observations: isize,
    pub lower_bound_by_acked_rate_factor: f64,
    pub hold_duration_factor: f64,
    pub use_byte_loss_rate: bool,
    pub padding_duration: TimeDelta,
    pub bound_best_candidate: bool,
    pub pace_at_loss_based_estimate: bool,
    pub median_sending_rate_factor: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bandwidth_rampup_upper_bound_factor: 0.0,
            bandwidth_rampup_upper_bound_factor_in_hold: 0.0,
            bandwidth_rampup_hold_threshold: 0.0,
            rampup_acceleration_max_factor: 0.0,
            rampup_acceleration_maxout_time: TimeDelta::Zero(),
            candidate_factors: Vec::new(),
            higher_bandwidth_bias_factor: 0.0,
            higher_log_bandwidth_bias_factor: 0.0,
            inherent_loss_lower_bound: 0.0,
            loss_threshold_of_high_bandwidth_preference: 0.0,
            bandwidth_preference_smoothing_factor: 0.0,
            inherent_loss_upper_bound_bandwidth_balance: DataRate::MinusInfinity(),
            inherent_loss_upper_bound_offset: 0.0,
            initial_inherent_loss_estimate: 0.0,
            newton_iterations: 0,
            newton_step_size: 0.0,
            append_acknowledged_rate_candidate: true,
            append_delay_based_estimate_candidate: false,
            append_upper_bound_candidate_in_alr: false,
            observation_duration_lower_bound: TimeDelta::Zero(),
            observation_window_size: 0,
            sending_rate_smoothing_factor: 0.0,
            instant_upper_bound_temporal_weight_factor: 0.0,
            instant_upper_bound_bandwidth_balance: DataRate::MinusInfinity(),
            instant_upper_bound_loss_offset: 0.0,
            temporal_weight_factor: 0.0,
            bandwidth_backoff_lower_bound_factor: 0.0,
            max_increase_factor: 0.0,
            delayed_increase_window: TimeDelta::Zero(),
            not_increase_if_inherent_loss_less_than_average_loss: false,
            not_use_acked_rate_in_alr: false,
            use_in_start_phase: false,
            min_num_observations: 0,
            lower_bound_by_acked_rate_factor: 0.0,
            hold_duration_factor: 0.0,
            use_byte_loss_rate: false,
            padding_duration: TimeDelta::Zero(),
            bound_best_candidate: false,
            pace_at_loss_based_estimate: false,
            median_sending_rate_factor: 0.0,
        }
    }
}

#[derive(Default)]
struct Derivatives {
    pub first: f64,
    pub second: f64,
}

struct Observation {
    pub num_packets: isize,
    pub num_lost_packets: isize,
    pub num_received_packets: isize,
    pub sending_rate: DataRate,
    pub size: DataSize,
    pub lost_size: DataSize,
    pub id: isize,
}

impl Default for Observation {
    fn default() -> Self {
        Self {
            num_packets: 0,
            num_lost_packets: 0,
            num_received_packets: 0,
            sending_rate: DataRate::MinusInfinity(),
            size: DataSize::Zero(),
            lost_size: DataSize::Zero(),
            id: -1,
        }
    }
}

impl Observation {
    pub fn IsInitialized(&self) -> bool {
        self.id != -1
    }
}

struct PartialObservation {
    pub num_packets: isize,
    pub lost_packets: HashMap<i64, DataSize>,
    pub size: DataSize,
}

impl Default for PartialObservation {
    fn default() -> Self {
        Self {
            num_packets: 0,
            lost_packets: HashMap::new(),
            size: DataSize::Zero(),
        }
    }
}

struct PaddingInfo {
    pub padding_rate: DataRate,
    pub padding_timestamp: Timestamp,
}

impl Default for PaddingInfo {
    fn default() -> Self {
        Self {
            padding_rate: DataRate::MinusInfinity(),
            padding_timestamp: Timestamp::MinusInfinity(),
        }
    }
}

struct HoldInfo {
    timestamp: Timestamp,
    duration: TimeDelta,
    rate: DataRate,
}

impl Default for HoldInfo {
    fn default() -> Self {
        Self {
            timestamp: Timestamp::MinusInfinity(),
            duration: TimeDelta::Zero(),
            rate: DataRate::PlusInfinity(),
        }
    }
}

pub struct LossBasedBweV2 {
    acknowledged_bitrate: Option<DataRate>,
    config: Option<Config>,
    current_best_estimate: ChannelParameters,
    num_observations: isize,
    observations: Vec<Observation>,
    partial_observation: PartialObservation,
    last_send_time_most_recent_observation: Timestamp,
    last_time_estimate_reduced: Timestamp,
    cached_instant_upper_bound: Option<DataRate>,
    cached_instant_lower_bound: Option<DataRate>,
    instant_upper_bound_temporal_weights: Vec<f64>,
    temporal_weights: Vec<f64>,
    recovering_after_loss_timestamp: Timestamp,
    bandwidth_limit_in_current_window: DataRate,
    min_bitrate: DataRate,
    max_bitrate: DataRate,
    delay_based_estimate: DataRate,
    loss_based_result: Result,
    last_hold_info: HoldInfo,
    last_padding_info: PaddingInfo,
    average_reported_loss_ratio: f64,
}

impl Default for LossBasedBweV2 {
    fn default() -> Self {
        Self {
            acknowledged_bitrate: None,
            config: None,
            current_best_estimate: ChannelParameters::default(),
            num_observations: 0,
            observations: Vec::new(),
            partial_observation: PartialObservation::default(),
            last_send_time_most_recent_observation: Timestamp::PlusInfinity(),
            last_time_estimate_reduced: Timestamp::MinusInfinity(),
            cached_instant_upper_bound: None,
            cached_instant_lower_bound: None,
            instant_upper_bound_temporal_weights: Vec::new(),
            temporal_weights: Vec::new(),
            recovering_after_loss_timestamp: Timestamp::MinusInfinity(),
            bandwidth_limit_in_current_window: DataRate::PlusInfinity(),
            min_bitrate: DataRate::KilobitsPerSec(1),
            max_bitrate: DataRate::PlusInfinity(),
            delay_based_estimate: DataRate::PlusInfinity(),
            loss_based_result: Result::default(),
            last_hold_info: HoldInfo::default(),
            last_padding_info: PaddingInfo::default(),
            average_reported_loss_ratio: 0.0,
        }
    }
}

impl LossBasedBweV2 {
    pub fn IsEnabled(&self) -> bool {
        todo!();
    }
    // Returns true iff a BWE can be calculated, i.e., the estimator has been
    // initialized with a BWE and then has received enough `PacketResult`s.
    pub fn IsReady(&self) -> bool {
        todo!();
    }

    // Returns true if loss based BWE is ready to be used in the start phase.
    pub fn ReadyToUseInStartPhase(&self) -> bool {
        todo!();
    }

    // Returns true if loss based BWE can be used in the start phase.
    pub fn UseInStartPhase(&self) -> bool {
        todo!();
    }

    // Returns `DataRate::PlusInfinity` if no BWE can be calculated.
    pub fn GetLossBasedResult(&self) -> Result {
        todo!();
    }

    pub fn SetAcknowledgedBitrate(&mut self, acknowledged_bitrate: DataRate) {
        todo!();
    }
    pub fn SetMinMaxBitrate(&mut self, min_bitrate: DataRate, max_bitrate: DataRate) {
        todo!();
    }
    pub fn UpdateBandwidthEstimate(
        &mut self,
        packet_results: &[PacketResult],
        delay_based_estimate: DataRate,
        in_alr: bool,
    ) {
        todo!();
    }
    pub fn PaceAtLossBasedEstimate(&self) -> bool {
        todo!();
    }

    // For unit testing only.
    pub fn SetBandwidthEstimate(&mut self, bandwidth_estimate: DataRate) {
        todo!();
    }

    fn CreateConfig() -> Option<Config> {
        todo!();
    }
    fn IsConfigValid(&self) -> bool {
        todo!();
    }

    // Returns `0.0` if not enough loss statistics have been received.
    fn UpdateAverageReportedLossRatio(&mut self) {
        todo!();
    }
    fn CalculateAverageReportedPacketLossRatio(&self) -> f64 {
        todo!();
    }
    // Calculates the average loss ratio over the last `observation_window_size`
    // observations but skips the observation with min and max loss ratio in order
    // to filter out loss spikes.
    fn CalculateAverageReportedByteLossRatio(&self) -> f64 {
        todo!();
    }
    fn GetCandidates(&self, in_alr: bool) -> Vec<ChannelParameters> {
        todo!();
    }
    fn GetCandidateBandwidthUpperBound(&self) -> DataRate {
        todo!();
    }
    fn GetDerivatives(&self, channel_parameters: &ChannelParameters) -> Derivatives {
        todo!();
    }
    fn GetFeasibleInherentLoss(&self, channel_parameters: &ChannelParameters) -> f64 {
        todo!();
    }
    fn GetInherentLossUpperBound(&self, bandwidth: DataRate) -> f64 {
        todo!();
    }
    fn AdjustBiasFactor(&self, loss_rate: f64, bias_factor: f64) -> f64 {
        todo!();
    }
    fn GetHighBandwidthBias(&self, bandwidth: DataRate) -> f64 {
        todo!();
    }
    fn GetObjective(&self, channel_parameters: &ChannelParameters) -> f64 {
        todo!();
    }
    fn GetSendingRate(&self, instantaneous_sending_rate: DataRate) -> DataRate {
        todo!();
    }
    fn GetInstantUpperBound(&self) -> DataRate {
        todo!();
    }
    fn CalculateInstantUpperBound(&mut self) {
        todo!();
    }
    fn GetInstantLowerBound(&self) -> DataRate {
        todo!();
    }
    fn CalculateInstantLowerBound(&mut self) {
        todo!();
    }

    fn CalculateTemporalWeights(&mut self) {
        todo!();
    }
    fn NewtonsMethodUpdate(&self, channel_parameters: &ChannelParameters) {
        todo!();
    }

    // Returns false if no observation was created.
    fn PushBackObservation(&mut self, packet_results: &[PacketResult]) -> bool {
        todo!();
    }
    fn IsEstimateIncreasingWhenLossLimited(
        &mut self,
        old_estimate: DataRate,
        new_estimate: DataRate,
    ) -> bool {
        todo!();
    }
    fn IsInLossLimitedState(&self) -> bool {
        todo!();
    }
    fn CanKeepIncreasingState(&self, estimate: DataRate) -> bool {
        todo!();
    }
    fn GetMedianSendingRate(&self) -> DataRate {
        todo!();
    }
}
