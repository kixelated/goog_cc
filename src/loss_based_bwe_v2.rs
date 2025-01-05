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
#[derive(Clone, Copy, Debug, PartialEq)]
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

#[derive(Clone, Debug)]
pub struct LossBasedBweV2Config {
    /// Enabled
    pub enabled: bool,
    /// BwRampupUpperBoundFactor
    pub bandwidth_rampup_upper_bound_factor: f64,
    /// BwRampupUpperBoundInHoldFactor
    pub bandwidth_rampup_upper_bound_factor_in_hold: f64,
    /// BwRampupUpperBoundHoldThreshold
    pub bandwidth_rampup_hold_threshold: f64,
    /// BwRampupAccelMaxFactor
    pub rampup_acceleration_max_factor: f64,
    /// BwRampupAccelMaxoutTime
    pub rampup_acceleration_maxout_time: TimeDelta,
    /// CandidateFactors
    pub candidate_factors: Vec<f64>,
    /// HigherBwBiasFactor
    pub higher_bandwidth_bias_factor: f64,
    /// HigherLogBwBiasFactor
    pub higher_log_bandwidth_bias_factor: f64,
    /// InherentLossLowerBound
    pub inherent_loss_lower_bound: f64,
    /// LossThresholdOfHighBandwidthPreference
    pub loss_threshold_of_high_bandwidth_preference: f64,
    /// BandwidthPreferenceSmoothingFactor
    pub bandwidth_preference_smoothing_factor: f64,
    /// InherentLossUpperBoundBwBalance
    pub inherent_loss_upper_bound_bandwidth_balance: DataRate,
    /// InherentLossUpperBoundOffset
    pub inherent_loss_upper_bound_offset: f64,
    /// InitialInherentLossEstimate
    pub initial_inherent_loss_estimate: f64,
    /// NewtonIterations
    pub newton_iterations: isize,
    /// NewtonStepSize
    pub newton_step_size: f64,
    /// AckedRateCandidate
    pub append_acknowledged_rate_candidate: bool,
    /// DelayBasedCandidate
    pub append_delay_based_estimate_candidate: bool,
    /// UpperBoundCandidateInAlr
    pub append_upper_bound_candidate_in_alr: bool,
    /// ObservationDurationLowerBound
    pub observation_duration_lower_bound: TimeDelta,
    /// ObservationWindowSize
    pub observation_window_size: isize,
    /// SendingRateSmoothingFactor
    pub sending_rate_smoothing_factor: f64,
    /// InstantUpperBoundTemporalWeightFactor
    pub instant_upper_bound_temporal_weight_factor: f64,
    /// InstantUpperBoundBwBalance
    pub instant_upper_bound_bandwidth_balance: DataRate,
    /// InstantUpperBoundLossOffset
    pub instant_upper_bound_loss_offset: f64,
    /// TemporalWeightFactor
    pub temporal_weight_factor: f64,
    /// BwBackoffLowerBoundFactor
    pub bandwidth_backoff_lower_bound_factor: f64,
    /// MaxIncreaseFactor
    pub max_increase_factor: f64,
    /// DelayedIncreaseWindow
    pub delayed_increase_window: TimeDelta,
    /// NotIncreaseIfInherentLossLessThanAverageLoss
    pub not_increase_if_inherent_loss_less_than_average_loss: bool,
    /// NotUseAckedRateInAlr
    pub not_use_acked_rate_in_alr: bool,
    /// UseInStartPhase
    pub use_in_start_phase: bool,
    /// MinNumObservations
    pub min_num_observations: isize,
    /// LowerBoundByAckedRateFactor
    pub lower_bound_by_acked_rate_factor: f64,
    /// HoldDurationFactor
    pub hold_duration_factor: f64,
    /// UseByteLossRate
    pub use_byte_loss_rate: bool,
    /// PaddingDuration
    pub padding_duration: TimeDelta,
    /// BoundBest
    pub bound_best_candidate: bool,
    /// PaceAtLossBasedEstimate
    pub pace_at_loss_based_estimate: bool,
    /// MedianSendingRateFactor
    pub median_sending_rate_factor: f64,
}

impl LossBasedBweV2Config {
    pub fn is_valid(&self) -> bool {
        let mut valid = true;

        if self.bandwidth_rampup_upper_bound_factor <= 1.0 {
            tracing::warn!(
                "The bandwidth rampup upper bound factor must be greater than 1: {}",
                self.bandwidth_rampup_upper_bound_factor
            );
            valid = false;
        }
        if self.bandwidth_rampup_upper_bound_factor_in_hold <= 1.0 {
            tracing::warn!(
                "The bandwidth rampup upper bound factor in hold must be greater than 1: {}",
                self.bandwidth_rampup_upper_bound_factor_in_hold
            );
            valid = false;
        }
        if self.bandwidth_rampup_hold_threshold < 0.0 {
            tracing::warn!(
                "The bandwidth rampup hold threshold must be non-negative: {}",
                self.bandwidth_rampup_hold_threshold
            );
            valid = false;
        }
        if self.rampup_acceleration_max_factor < 0.0 {
            tracing::warn!(
                "The rampup acceleration max factor must be non-negative: {}",
                self.rampup_acceleration_max_factor
            );
            valid = false;
        }
        if self.rampup_acceleration_maxout_time <= TimeDelta::Zero() {
            tracing::warn!(
                "The rampup acceleration maxout time must be above zero: {:?}",
                self.rampup_acceleration_maxout_time
            );
            valid = false;
        }
        for candidate_factor in self.candidate_factors.iter() {
            if *candidate_factor <= 0.0 {
                tracing::warn!(
                    "All candidate factors must be greater than zero: {}",
                    candidate_factor
                );
                valid = false;
            }
        }

        // Ensure that the configuration allows generation of at least one candidate
        // other than the current estimate.
        if !self.append_acknowledged_rate_candidate
            && !self.append_delay_based_estimate_candidate
            && self.candidate_factors.iter().any(|cf| *cf != 1.0)
        {
            tracing::warn!("The configuration does not allow generating candidates. Specify a candidate factor other than 1.0, allow the acknowledged rate to be a candidate, and/or allow the delay based estimate to be a candidate.");
            valid = false;
        }

        if self.higher_bandwidth_bias_factor < 0.0 {
            tracing::warn!(
                "The higher bandwidth bias factor must be non-negative: {}",
                self.higher_bandwidth_bias_factor
            );
            valid = false;
        }
        if self.inherent_loss_lower_bound < 0.0 || self.inherent_loss_lower_bound >= 1.0 {
            tracing::warn!(
                "The inherent loss lower bound must be in [0, 1): {}",
                self.inherent_loss_lower_bound
            );
            valid = false;
        }
        if self.loss_threshold_of_high_bandwidth_preference < 0.0
            || self.loss_threshold_of_high_bandwidth_preference >= 1.0
        {
            tracing::warn!(
                "The loss threshold of high bandwidth preference must be in [0, 1): {}",
                self.loss_threshold_of_high_bandwidth_preference
            );
            valid = false;
        }
        if self.bandwidth_preference_smoothing_factor <= 0.0
            || self.bandwidth_preference_smoothing_factor > 1.0
        {
            tracing::warn!(
                "The bandwidth preference smoothing factor must be in (0, 1]: {}",
                self.bandwidth_preference_smoothing_factor
            );
            valid = false;
        }
        if self.inherent_loss_upper_bound_bandwidth_balance <= DataRate::Zero() {
            tracing::warn!(
                "The inherent loss upper bound bandwidth balance must be positive: {:?}",
                self.inherent_loss_upper_bound_bandwidth_balance
            );
            valid = false;
        }
        if self.inherent_loss_upper_bound_offset < self.inherent_loss_lower_bound
            || self.inherent_loss_upper_bound_offset >= 1.0
        {
            tracing::warn!("The inherent loss upper bound must be greater than or equal to the inherent loss lower bound, which is {}, and less than 1: {}", self.inherent_loss_lower_bound, self.inherent_loss_upper_bound_offset);
            valid = false;
        }
        if self.initial_inherent_loss_estimate < 0.0 || self.initial_inherent_loss_estimate >= 1.0 {
            tracing::warn!(
                "The initial inherent loss estimate must be in [0, 1): {}",
                self.initial_inherent_loss_estimate
            );
            valid = false;
        }
        if self.newton_iterations <= 0 {
            tracing::warn!(
                "The number of Newton iterations must be positive: {}",
                self.newton_iterations
            );
            valid = false;
        }
        if self.newton_step_size <= 0.0 {
            tracing::warn!(
                "The Newton step size must be positive: {}",
                self.newton_step_size
            );
            valid = false;
        }
        if self.observation_duration_lower_bound <= TimeDelta::Zero() {
            tracing::warn!(
                "The observation duration lower bound must be positive: {}",
                self.observation_duration_lower_bound.ms()
            );
            valid = false;
        }
        if self.observation_window_size < 2 {
            tracing::warn!(
                "The observation window size must be at least 2: {}",
                self.observation_window_size
            );
            valid = false;
        }
        if self.sending_rate_smoothing_factor < 0.0 || self.sending_rate_smoothing_factor >= 1.0 {
            tracing::warn!(
                "The sending rate smoothing factor must be in [0, 1): {}",
                self.sending_rate_smoothing_factor
            );
            valid = false;
        }
        if self.instant_upper_bound_temporal_weight_factor <= 0.0
            || self.instant_upper_bound_temporal_weight_factor > 1.0
        {
            tracing::warn!(
                "The instant upper bound temporal weight factor must be in (0, 1]: {}",
                self.instant_upper_bound_temporal_weight_factor
            );
            valid = false;
        }
        if self.instant_upper_bound_bandwidth_balance <= DataRate::Zero() {
            tracing::warn!(
                "The instant upper bound bandwidth balance must be positive: {:?}",
                self.instant_upper_bound_bandwidth_balance
            );
            valid = false;
        }
        if self.instant_upper_bound_loss_offset < 0.0 || self.instant_upper_bound_loss_offset >= 1.0
        {
            tracing::warn!(
                "The instant upper bound loss offset must be in [0, 1): {}",
                self.instant_upper_bound_loss_offset
            );
            valid = false;
        }
        if self.temporal_weight_factor <= 0.0 || self.temporal_weight_factor > 1.0 {
            tracing::warn!(
                "The temporal weight factor must be in (0, 1]: {}",
                self.temporal_weight_factor
            );
            valid = false;
        }
        if self.bandwidth_backoff_lower_bound_factor > 1.0 {
            tracing::warn!(
                "The bandwidth backoff lower bound factor must not be greater than 1: {}",
                self.bandwidth_backoff_lower_bound_factor
            );
            valid = false;
        }
        if self.max_increase_factor <= 0.0 {
            tracing::warn!(
                "The maximum increase factor must be positive: {}",
                self.max_increase_factor
            );
            valid = false;
        }
        if self.delayed_increase_window <= TimeDelta::Zero() {
            tracing::warn!(
                "The delayed increase window must be positive: {:?}",
                self.delayed_increase_window
            );
            valid = false;
        }
        if self.min_num_observations <= 0 {
            tracing::warn!(
                "The min number of observations must be positive: {}",
                self.min_num_observations
            );
            valid = false;
        }
        if self.lower_bound_by_acked_rate_factor < 0.0 {
            tracing::warn!(
                "The estimate lower bound by acknowledged rate factor must be non-negative: {}",
                self.lower_bound_by_acked_rate_factor
            );
            valid = false;
        }
        valid
    }
}

impl Default for LossBasedBweV2Config {
    fn default() -> Self {
        Self {
            enabled: true,                                                             // Enabled
            bandwidth_rampup_upper_bound_factor: 1000000.0, // BwRampupUpperBoundFactor
            bandwidth_rampup_upper_bound_factor_in_hold: 1000000.0, // BwRampupUpperBoundInHoldFactor
            bandwidth_rampup_hold_threshold: 1.3, // BwRampupUpperBoundHoldThreshold
            rampup_acceleration_max_factor: 0.0,  // BwRampupAccelMaxFactor
            rampup_acceleration_maxout_time: TimeDelta::Seconds(60), // BwRampupAccelMaxoutTime
            candidate_factors: vec![1.02, 1.0, 0.95], // CandidateFactors
            higher_bandwidth_bias_factor: 0.0002, // HigherBwBiasFactor
            higher_log_bandwidth_bias_factor: 0.02, // HigherLogBwBiasFactor
            inherent_loss_lower_bound: 1.0e-3,    // InherentLossLowerBound
            loss_threshold_of_high_bandwidth_preference: 0.15, // LossThresholdOfHighBandwidthPreference
            bandwidth_preference_smoothing_factor: 0.002,      // BandwidthPreferenceSmoothingFactor
            inherent_loss_upper_bound_bandwidth_balance: DataRate::KilobitsPerSec(75), // InherentLossUpperBoundBwBalance
            inherent_loss_upper_bound_offset: 0.05, // InherentLossUpperBoundOffset
            initial_inherent_loss_estimate: 0.01,   // InitialInherentLossEstimate
            newton_iterations: 1,                   // NewtonIterations
            newton_step_size: 0.75,                 // NewtonStepSize
            append_acknowledged_rate_candidate: true, // AckedRateCandidate
            append_delay_based_estimate_candidate: true, // DelayBasedCandidate
            append_upper_bound_candidate_in_alr: false, // UpperBoundCandidateInAlr
            observation_duration_lower_bound: TimeDelta::Millis(250), // ObservationDurationLowerBound
            observation_window_size: 20,                              // ObservationWindowSize
            sending_rate_smoothing_factor: 0.0,                       // SendingRateSmoothingFactor
            instant_upper_bound_temporal_weight_factor: 0.9, // InstantUpperBoundTemporalWeightFactor
            instant_upper_bound_bandwidth_balance: DataRate::KilobitsPerSec(75), // InstantUpperBoundBwBalance
            instant_upper_bound_loss_offset: 0.05, // InstantUpperBoundLossOffset
            temporal_weight_factor: 0.9,           // TemporalWeightFactor
            bandwidth_backoff_lower_bound_factor: 1.0, // BwBackoffLowerBoundFactor
            max_increase_factor: 1.3,              // MaxIncreaseFactor
            delayed_increase_window: TimeDelta::Millis(300), // DelayedIncreaseWindow
            not_increase_if_inherent_loss_less_than_average_loss: true, // NotIncreaseIfInherentLossLessThanAverageLoss
            not_use_acked_rate_in_alr: true,                            // NotUseAckedRateInAlr
            use_in_start_phase: false,                                  // UseInStartPhase
            min_num_observations: 3,                                    // MinNumObservations
            lower_bound_by_acked_rate_factor: 0.0, // LowerBoundByAckedRateFactor
            hold_duration_factor: 0.0,             // HoldDurationFactor
            use_byte_loss_rate: false,             // UseByteLossRate
            padding_duration: TimeDelta::Zero(),   // PaddingDuration
            bound_best_candidate: false,           // BoundBest
            pace_at_loss_based_estimate: false,    // PaceAtLossBasedEstimate
            median_sending_rate_factor: 2.0,       // MedianSendingRateFactor
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
    config: Option<LossBasedBweV2Config>,
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

    fn IsConfigValid(&self) -> bool {
        self.config
            .as_ref()
            .map_or(false, LossBasedBweV2Config::is_valid)
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
