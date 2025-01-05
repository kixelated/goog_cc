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

use crate::{
    api::{
        transport::PacketResult,
        units::{DataRate, DataSize, TimeDelta, Timestamp},
    },
    remote_bitrate_estimator::CongestionControllerMinBitrate,
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
#[derive(Clone, Debug, Copy)]
pub struct LossBasedBweV2Result {
    pub bandwidth_estimate: DataRate,
    // State is used by goog_cc, which later sends probe requests to probe
    // controller if state is Increasing.
    pub state: LossBasedState,
}

impl Default for LossBasedBweV2Result {
    fn default() -> Self {
        Self {
            bandwidth_estimate: DataRate::Zero(),
            state: LossBasedState::DelayBasedEstimate,
        }
    }
}

#[derive(Clone, Debug)]
struct ChannelParameters {
    pub inherent_loss: f64,
    pub loss_limited_bandwidth: DataRate,
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
    pub newton_iterations: usize,
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
    pub observation_window_size: usize,
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
    pub min_num_observations: usize,
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

#[derive(Debug, Clone)]
struct Observation {
    pub num_packets: usize,
    pub num_lost_packets: usize,
    pub num_received_packets: usize,
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
    pub num_packets: usize,
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
    config: LossBasedBweV2Config,
    current_best_estimate: ChannelParameters,
    num_observations: usize,
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
    loss_based_result: LossBasedBweV2Result,
    last_hold_info: HoldInfo,
    last_padding_info: PaddingInfo,
    average_reported_loss_ratio: f64,
}

impl LossBasedBweV2 {
    const InitHoldDuration: TimeDelta = TimeDelta::Millis(300);
    const MaxHoldDuration: TimeDelta = TimeDelta::Seconds(60);

    pub fn new(config: LossBasedBweV2Config) -> Self {
        assert!(config.enabled);
        assert!(config.is_valid());

        let mut temporal_weights = Vec::with_capacity(config.observation_window_size);
        let mut instant_upper_bound_temporal_weights =
            Vec::with_capacity(config.observation_window_size);

        for i in 0..config.observation_window_size {
            temporal_weights[i] = config.temporal_weight_factor.powf(i as _);
            instant_upper_bound_temporal_weights[i] = config
                .instant_upper_bound_temporal_weight_factor
                .powf(i as _);
        }

        let mut last_hold_info = HoldInfo::default();
        last_hold_info.duration = Self::InitHoldDuration;

        Self {
            acknowledged_bitrate: None,
            current_best_estimate: ChannelParameters::default(),
            num_observations: 0,
            observations: vec![Observation::default(); config.observation_window_size],
            partial_observation: PartialObservation::default(),
            last_send_time_most_recent_observation: Timestamp::PlusInfinity(),
            last_time_estimate_reduced: Timestamp::MinusInfinity(),
            cached_instant_upper_bound: None,
            cached_instant_lower_bound: None,
            temporal_weights,
            instant_upper_bound_temporal_weights,
            recovering_after_loss_timestamp: Timestamp::MinusInfinity(),
            bandwidth_limit_in_current_window: DataRate::PlusInfinity(),
            min_bitrate: DataRate::KilobitsPerSec(1),
            max_bitrate: DataRate::PlusInfinity(),
            delay_based_estimate: DataRate::PlusInfinity(),
            loss_based_result: LossBasedBweV2Result::default(),
            last_hold_info,
            last_padding_info: PaddingInfo::default(),
            average_reported_loss_ratio: 0.0,
            config,
        }
    }

    pub fn IsEnabled(&self) -> bool {
        self.config.enabled
    }
    // Returns true iff a BWE can be calculated, i.e., the estimator has been
    // initialized with a BWE and then has received enough `PacketResult`s.
    pub fn IsReady(&self) -> bool {
        self.IsEnabled()
            && self.current_best_estimate.loss_limited_bandwidth.IsFinite()
            && self.num_observations >= self.config.min_num_observations
    }

    // Returns true if loss based BWE is ready to be used in the start phase.
    pub fn ReadyToUseInStartPhase(&self) -> bool {
        self.IsReady() && self.config.use_in_start_phase
    }

    // Returns true if loss based BWE can be used in the start phase.
    pub fn UseInStartPhase(&self) -> bool {
        self.config.use_in_start_phase
    }

    // Returns `DataRate::PlusInfinity` if no BWE can be calculated.
    pub fn GetLossBasedResult(&self) -> LossBasedBweV2Result {
        if !self.IsReady() {
            if !self.IsEnabled() {
                tracing::warn!("The estimator must be enabled before it can be used.");
            } else {
                if !self.current_best_estimate.loss_limited_bandwidth.IsFinite() {
                    tracing::warn!("The estimator must be initialized before it can be used.");
                }
                if self.num_observations <= self.config.min_num_observations {
                    tracing::warn!(
                        "The estimator must receive enough loss statistics before it can be used."
                    );
                }
            }
            return LossBasedBweV2Result {
                bandwidth_estimate: if self.delay_based_estimate.IsFinite() {
                    self.delay_based_estimate
                } else {
                    DataRate::PlusInfinity()
                },
                state: LossBasedState::DelayBasedEstimate,
            };
        }
        self.loss_based_result
    }

    pub fn SetAcknowledgedBitrate(&mut self, acknowledged_bitrate: DataRate) {
        if acknowledged_bitrate.IsFinite() {
            self.acknowledged_bitrate = Some(acknowledged_bitrate);
            self.CalculateInstantLowerBound();
        } else {
            tracing::warn!(
                "The acknowledged bitrate must be finite: {:?}",
                acknowledged_bitrate
            );
        }
    }
    pub fn SetMinMaxBitrate(&mut self, min_bitrate: DataRate, max_bitrate: DataRate) {
        if min_bitrate.IsFinite() {
            self.min_bitrate = min_bitrate;
            self.CalculateInstantLowerBound();
        } else {
            tracing::warn!("The min bitrate must be finite: {:?}", min_bitrate);
        }

        if max_bitrate.IsFinite() {
            self.max_bitrate = max_bitrate;
        } else {
            tracing::warn!("The max bitrate must be finite: {:?}", max_bitrate);
        }
    }
    pub fn UpdateBandwidthEstimate(
        &mut self,
        packet_results: &[PacketResult],
        delay_based_estimate: DataRate,
        in_alr: bool,
    ) {
        self.delay_based_estimate = delay_based_estimate;
        if !self.IsEnabled() {
            tracing::warn!("The estimator must be enabled before it can be used.");
            return;
        }

        if packet_results.is_empty() {
            tracing::debug!("The estimate cannot be updated without any loss statistics.");
            return;
        }

        if !self.PushBackObservation(packet_results) {
            return;
        }

        if !self.current_best_estimate.loss_limited_bandwidth.IsFinite() {
            if !delay_based_estimate.IsFinite() {
                tracing::warn!(
                    "The delay based estimate must be finite: {:?}",
                    delay_based_estimate
                );
                return;
            }
            self.current_best_estimate.loss_limited_bandwidth = delay_based_estimate;
            self.loss_based_result = LossBasedBweV2Result {
                bandwidth_estimate: delay_based_estimate,
                state: LossBasedState::DelayBasedEstimate,
            };
        }

        let mut best_candidate: ChannelParameters = self.current_best_estimate.clone();
        let mut objective_max: f64 = f64::MIN;
        for mut candidate in self.GetCandidates(in_alr) {
            self.NewtonsMethodUpdate(&mut candidate);

            let candidate_objective: f64 = self.GetObjective(&candidate);
            if candidate_objective > objective_max {
                objective_max = candidate_objective;
                best_candidate = candidate;
            }
        }
        if best_candidate.loss_limited_bandwidth
            < self.current_best_estimate.loss_limited_bandwidth
        {
            self.last_time_estimate_reduced = self.last_send_time_most_recent_observation;
        }

        // Do not increase the estimate if the average loss is greater than current
        // inherent loss.
        if self.average_reported_loss_ratio > best_candidate.inherent_loss
            && self
                .config
                .not_increase_if_inherent_loss_less_than_average_loss
            && self.current_best_estimate.loss_limited_bandwidth
                < best_candidate.loss_limited_bandwidth
        {
            best_candidate.loss_limited_bandwidth =
                self.current_best_estimate.loss_limited_bandwidth;
        }

        if self.IsInLossLimitedState() {
            // Bound the estimate increase if:
            // 1. The estimate has been increased for less than
            // `delayed_increase_window` ago, and
            // 2. The best candidate is greater than bandwidth_limit_in_current_window.
            if self.recovering_after_loss_timestamp.IsFinite()
                && self.recovering_after_loss_timestamp + self.config.delayed_increase_window
                    > self.last_send_time_most_recent_observation
                && best_candidate.loss_limited_bandwidth > self.bandwidth_limit_in_current_window
            {
                best_candidate.loss_limited_bandwidth = self.bandwidth_limit_in_current_window;
            }

            let increasing_when_loss_limited: bool = self.IsEstimateIncreasingWhenLossLimited(
                /*old_estimate=*/ self.current_best_estimate.loss_limited_bandwidth,
                /*new_estimate=*/ best_candidate.loss_limited_bandwidth,
            );
            // Bound the best candidate by the acked bitrate.
            if let Some(acknowledged_bitrate) = self.acknowledged_bitrate {
                if increasing_when_loss_limited && acknowledged_bitrate.IsFinite() {
                    let mut rampup_factor: f64 = self.config.bandwidth_rampup_upper_bound_factor;
                    if self.last_hold_info.rate.IsFinite()
                        && acknowledged_bitrate
                            < self.config.bandwidth_rampup_hold_threshold
                                * self.last_hold_info.rate
                    {
                        rampup_factor = self.config.bandwidth_rampup_upper_bound_factor_in_hold;
                    }

                    best_candidate.loss_limited_bandwidth = std::cmp::max(
                        self.current_best_estimate.loss_limited_bandwidth,
                        std::cmp::min(
                            best_candidate.loss_limited_bandwidth,
                            rampup_factor * (acknowledged_bitrate),
                        ),
                    );
                    // Increase current estimate by at least 1kbps to make sure that the state
                    // will be switched to Increasing, thus padding is triggered.
                    if self.loss_based_result.state == LossBasedState::Decreasing
                        && best_candidate.loss_limited_bandwidth
                            == self.current_best_estimate.loss_limited_bandwidth
                    {
                        best_candidate.loss_limited_bandwidth =
                            self.current_best_estimate.loss_limited_bandwidth
                                + DataRate::BitsPerSec(1);
                    }
                }
            }
        }

        let mut bounded_bandwidth_estimate: DataRate = DataRate::PlusInfinity();
        if self.delay_based_estimate.IsFinite() {
            bounded_bandwidth_estimate = self.GetInstantLowerBound().max(
                [
                    best_candidate.loss_limited_bandwidth,
                    self.GetInstantUpperBound(),
                    self.delay_based_estimate,
                ]
                .iter()
                .min()
                .cloned()
                .unwrap(),
            );
        } else {
            bounded_bandwidth_estimate = std::cmp::max(
                self.GetInstantLowerBound(),
                std::cmp::min(
                    best_candidate.loss_limited_bandwidth,
                    self.GetInstantUpperBound(),
                ),
            );
        }
        if self.config.bound_best_candidate
            && bounded_bandwidth_estimate < best_candidate.loss_limited_bandwidth
        {
            tracing::info!(
                "Resetting loss based BWE to {:?} due to loss. Avg loss rate: {:?}",
                bounded_bandwidth_estimate,
                self.average_reported_loss_ratio
            );
            self.current_best_estimate.loss_limited_bandwidth = bounded_bandwidth_estimate;
            self.current_best_estimate.inherent_loss = 0.0;
        } else {
            self.current_best_estimate = best_candidate;
            if self.config.lower_bound_by_acked_rate_factor > 0.0 {
                self.current_best_estimate.loss_limited_bandwidth = std::cmp::max(
                    self.current_best_estimate.loss_limited_bandwidth,
                    self.GetInstantLowerBound(),
                );
            }
        }

        if self.loss_based_result.state == LossBasedState::Decreasing
            && self.last_hold_info.timestamp > self.last_send_time_most_recent_observation
            && bounded_bandwidth_estimate < self.delay_based_estimate
        {
            // Ensure that acked rate is the lower bound of HOLD rate.
            if self.config.lower_bound_by_acked_rate_factor > 0.0 {
                self.last_hold_info.rate =
                    std::cmp::max(self.GetInstantLowerBound(), self.last_hold_info.rate);
            }
            // BWE is not allowed to increase above the HOLD rate. The purpose of
            // HOLD is to not immediately ramp up BWE to a rate that may cause loss.
            self.loss_based_result.bandwidth_estimate =
                std::cmp::min(self.last_hold_info.rate, bounded_bandwidth_estimate);
            return;
        }

        if self.IsEstimateIncreasingWhenLossLimited(
            /*old_estimate=*/ self.loss_based_result.bandwidth_estimate,
            /*new_estimate=*/ bounded_bandwidth_estimate,
        ) && self.CanKeepIncreasingState(bounded_bandwidth_estimate)
            && bounded_bandwidth_estimate < self.delay_based_estimate
            && bounded_bandwidth_estimate < self.max_bitrate
        {
            if self.config.padding_duration > TimeDelta::Zero()
                && bounded_bandwidth_estimate > self.last_padding_info.padding_rate
            {
                // Start a new padding duration.
                self.last_padding_info.padding_rate = bounded_bandwidth_estimate;
                self.last_padding_info.padding_timestamp =
                    self.last_send_time_most_recent_observation;
            }
            self.loss_based_result.state = if self.config.padding_duration > TimeDelta::Zero() {
                LossBasedState::IncreaseUsingPadding
            } else {
                LossBasedState::Increasing
            };
        } else if bounded_bandwidth_estimate < self.delay_based_estimate
            && bounded_bandwidth_estimate < self.max_bitrate
        {
            if self.loss_based_result.state != LossBasedState::Decreasing
                && self.config.hold_duration_factor > 0.0
            {
                tracing::info!(
                    "Switch to HOLD. Bounded BWE: {:?}, duration: {:?}",
                    bounded_bandwidth_estimate,
                    self.last_hold_info.duration
                );
                self.last_hold_info = HoldInfo {
                    timestamp: self.last_send_time_most_recent_observation
                        + self.last_hold_info.duration,
                    duration: std::cmp::min(
                        Self::MaxHoldDuration,
                        self.last_hold_info.duration * self.config.hold_duration_factor,
                    ),
                    rate: bounded_bandwidth_estimate,
                };
            }
            self.last_padding_info = PaddingInfo::default();
            self.loss_based_result.state = LossBasedState::Decreasing;
        } else {
            // Reset the HOLD info if delay based estimate works to avoid getting
            // stuck in low bitrate.
            self.last_hold_info = HoldInfo {
                timestamp: Timestamp::MinusInfinity(),
                duration: Self::InitHoldDuration,
                rate: DataRate::PlusInfinity(),
            };
            self.last_padding_info = PaddingInfo::default();
            self.loss_based_result.state = LossBasedState::DelayBasedEstimate;
        }
        self.loss_based_result.bandwidth_estimate = bounded_bandwidth_estimate;

        if self.IsInLossLimitedState()
            && (self.recovering_after_loss_timestamp.IsInfinite()
                || self.recovering_after_loss_timestamp + self.config.delayed_increase_window
                    < self.last_send_time_most_recent_observation)
        {
            self.bandwidth_limit_in_current_window = std::cmp::max(
                CongestionControllerMinBitrate,
                self.current_best_estimate.loss_limited_bandwidth * self.config.max_increase_factor,
            );
            self.recovering_after_loss_timestamp = self.last_send_time_most_recent_observation;
        }
    }
    pub fn PaceAtLossBasedEstimate(&self) -> bool {
        self.config.pace_at_loss_based_estimate
            && self.loss_based_result.state != LossBasedState::DelayBasedEstimate
    }

    // For unit testing only.
    pub fn SetBandwidthEstimate(&mut self, bandwidth_estimate: DataRate) {
        if bandwidth_estimate.IsFinite() {
            self.current_best_estimate.loss_limited_bandwidth = bandwidth_estimate;
            self.loss_based_result = LossBasedBweV2Result {
                bandwidth_estimate,
                state: LossBasedState::DelayBasedEstimate,
            };
        } else {
            tracing::warn!(
                "The bandwidth estimate must be finite: {:?}",
                bandwidth_estimate
            );
        }
    }

    fn IsConfigValid(&self) -> bool {
        self.config.is_valid()
    }

    // Returns `0.0` if not enough loss statistics have been received.
    fn UpdateAverageReportedLossRatio(&mut self) {
        self.average_reported_loss_ratio = if self.config.use_byte_loss_rate {
            self.CalculateAverageReportedByteLossRatio()
        } else {
            self.CalculateAverageReportedPacketLossRatio()
        };
    }
    fn CalculateAverageReportedPacketLossRatio(&self) -> f64 {
        if self.num_observations <= 0 {
            return 0.0;
        }

        let mut num_packets: f64 = 0.0;
        let mut num_lost_packets: f64 = 0.0;
        for observation in &self.observations {
            if !observation.IsInitialized() {
                continue;
            }

            let id: usize = observation.id.try_into().unwrap();
            let instant_temporal_weight: f64 =
                self.instant_upper_bound_temporal_weights[(self.num_observations - 1) - id];
            num_packets += instant_temporal_weight * observation.num_packets as f64;
            num_lost_packets += instant_temporal_weight * observation.num_lost_packets as f64;
        }

        num_lost_packets / num_packets
    }
    // Calculates the average loss ratio over the last `observation_window_size`
    // observations but skips the observation with min and max loss ratio in order
    // to filter out loss spikes.
    fn CalculateAverageReportedByteLossRatio(&self) -> f64 {
        if self.num_observations <= 0 {
            return 0.0;
        }

        let mut total_bytes: DataSize = DataSize::Zero();
        let mut lost_bytes: DataSize = DataSize::Zero();
        let mut min_loss_rate: f64 = 1.0;
        let mut max_loss_rate: f64 = 0.0;
        let mut min_lost_bytes: DataSize = DataSize::Zero();
        let mut max_lost_bytes: DataSize = DataSize::Zero();
        let mut min_bytes_received: DataSize = DataSize::Zero();
        let mut max_bytes_received: DataSize = DataSize::Zero();
        let mut send_rate_of_max_loss_observation: DataRate = DataRate::Zero();
        for observation in &self.observations {
            if !observation.IsInitialized() {
                continue;
            }

            let id: usize = observation.id.try_into().unwrap();
            let instant_temporal_weight: f64 =
                self.instant_upper_bound_temporal_weights[(self.num_observations - 1) - id];
            total_bytes += instant_temporal_weight * observation.size;
            lost_bytes += instant_temporal_weight * observation.lost_size;

            let loss_rate: f64 = if !observation.size.IsZero() {
                observation.lost_size / observation.size
            } else {
                0.0
            };
            if self.num_observations > 3 {
                if loss_rate > max_loss_rate {
                    max_loss_rate = loss_rate;
                    max_lost_bytes = instant_temporal_weight * observation.lost_size;
                    max_bytes_received = instant_temporal_weight * observation.size;
                    send_rate_of_max_loss_observation = observation.sending_rate;
                }
                if loss_rate < min_loss_rate {
                    min_loss_rate = loss_rate;
                    min_lost_bytes = instant_temporal_weight * observation.lost_size;
                    min_bytes_received = instant_temporal_weight * observation.size;
                }
            }
        }
        if self.GetMedianSendingRate() * self.config.median_sending_rate_factor
            <= send_rate_of_max_loss_observation
        {
            // If the median sending rate is less than half of the sending rate of the
            // observation with max loss rate, i.e. we suddenly send a lot of data, then
            // the loss rate might not be due to a spike.
            return lost_bytes / total_bytes;
        }
        (lost_bytes - min_lost_bytes - max_lost_bytes)
            / (total_bytes - max_bytes_received - min_bytes_received)
    }
    fn GetCandidates(&self, in_alr: bool) -> Vec<ChannelParameters> {
        let best_estimate: ChannelParameters = self.current_best_estimate.clone();
        let mut bandwidths: Vec<DataRate> = Vec::new();
        for candidate_factor in &self.config.candidate_factors {
            bandwidths.push(*candidate_factor * best_estimate.loss_limited_bandwidth);
        }

        if let Some(acknowledged_bitrate) = self.acknowledged_bitrate {
            if self.config.append_acknowledged_rate_candidate && (!(self.config.not_use_acked_rate_in_alr && in_alr) || (self.config.padding_duration > TimeDelta::Zero()
                        && self.last_padding_info.padding_timestamp + self.config.padding_duration
                            >= self.last_send_time_most_recent_observation)) {
                bandwidths.push(
                    acknowledged_bitrate * self.config.bandwidth_backoff_lower_bound_factor,
                );
            }
        }

        if self.delay_based_estimate.IsFinite() && self.config.append_delay_based_estimate_candidate && self.delay_based_estimate > best_estimate.loss_limited_bandwidth {
            bandwidths.push(self.delay_based_estimate);
        }

        if in_alr
            && self.config.append_upper_bound_candidate_in_alr
            && best_estimate.loss_limited_bandwidth > self.GetInstantUpperBound()
        {
            bandwidths.push(self.GetInstantUpperBound());
        }

        let candidate_bandwidth_upper_bound: DataRate = self.GetCandidateBandwidthUpperBound();

        let mut candidates: Vec<ChannelParameters> = Vec::new();
        for i in 0..bandwidths.len() {
            let mut candidate: ChannelParameters = best_estimate.clone();
            candidate.loss_limited_bandwidth = std::cmp::min(
                bandwidths[i],
                std::cmp::max(
                    best_estimate.loss_limited_bandwidth,
                    candidate_bandwidth_upper_bound,
                ),
            );
            candidate.inherent_loss = self.GetFeasibleInherentLoss(&candidate);
            candidates.push(candidate);
        }
        candidates
    }
    fn GetCandidateBandwidthUpperBound(&self) -> DataRate {
        let mut candidate_bandwidth_upper_bound: DataRate = self.max_bitrate;
        if self.IsInLossLimitedState() && self.bandwidth_limit_in_current_window.IsFinite() {
            candidate_bandwidth_upper_bound = self.bandwidth_limit_in_current_window;
        }

        let acknowledged_bitrate = match self.acknowledged_bitrate {
            Some(rate) => rate,
            None => return candidate_bandwidth_upper_bound,
        };

        if self.config.rampup_acceleration_max_factor > 0.0 {
            let time_since_bandwidth_reduced: TimeDelta = std::cmp::min(
                self.config.rampup_acceleration_maxout_time,
                std::cmp::max(
                    TimeDelta::Zero(),
                    self.last_send_time_most_recent_observation - self.last_time_estimate_reduced,
                ),
            );
            let rampup_acceleration: f64 = self.config.rampup_acceleration_max_factor
                * time_since_bandwidth_reduced
                / self.config.rampup_acceleration_maxout_time;

            candidate_bandwidth_upper_bound += rampup_acceleration * (acknowledged_bitrate);
        }
        candidate_bandwidth_upper_bound
    }
    fn GetDerivatives(&self, channel_parameters: &ChannelParameters) -> Derivatives {
        let mut derivatives = Derivatives::default();

        for observation in &self.observations {
            if !observation.IsInitialized() {
                continue;
            }

            let loss_probability: f64 = GetLossProbability(
                channel_parameters.inherent_loss,
                channel_parameters.loss_limited_bandwidth,
                observation.sending_rate,
            );

            let id: usize = observation.id.try_into().unwrap();
            let temporal_weight: f64 = self.temporal_weights[(self.num_observations - 1) - id];
            if self.config.use_byte_loss_rate {
                derivatives.first += temporal_weight
                    * ((ToKiloBytes(observation.lost_size) / loss_probability)
                        - (ToKiloBytes(observation.size - observation.lost_size)
                            / (1.0 - loss_probability)));
                derivatives.second -= temporal_weight
                    * ((ToKiloBytes(observation.lost_size) / loss_probability.powf(2.0))
                        + (ToKiloBytes(observation.size - observation.lost_size)
                            / (1.0 - loss_probability).powf(2.0)));
            } else {
                derivatives.first += temporal_weight
                    * ((observation.num_lost_packets as f64 / loss_probability)
                        - (observation.num_received_packets as f64 / (1.0 - loss_probability)));
                derivatives.second -= temporal_weight
                    * ((observation.num_lost_packets as f64 / loss_probability.powf(2.0))
                        + (observation.num_received_packets as f64
                            / (1.0 - loss_probability).powf(2.0)));
            }
        }

        if derivatives.second >= 0.0 {
            tracing::error!(
                "The second derivative is mathematically guaranteed to be negative but is {}.",
                derivatives.second
            );
            derivatives.second = -1.0e-6;
        }

        derivatives
    }
    fn GetFeasibleInherentLoss(&self, channel_parameters: &ChannelParameters) -> f64 {
        channel_parameters
            .inherent_loss
            .max(self.config.inherent_loss_lower_bound)
            .min(self.GetInherentLossUpperBound(channel_parameters.loss_limited_bandwidth))
    }
    fn GetInherentLossUpperBound(&self, bandwidth: DataRate) -> f64 {
        if bandwidth.IsZero() {
            return 1.0;
        }

        let inherent_loss_upper_bound: f64 = self.config.inherent_loss_upper_bound_offset
            + self.config.inherent_loss_upper_bound_bandwidth_balance / bandwidth;
        inherent_loss_upper_bound.min(1.0)
    }
    fn AdjustBiasFactor(&self, loss_rate: f64, bias_factor: f64) -> f64 {
        bias_factor * (self.config.loss_threshold_of_high_bandwidth_preference - loss_rate)
            / (self.config.bandwidth_preference_smoothing_factor
                + (self.config.loss_threshold_of_high_bandwidth_preference - loss_rate).abs())
    }
    fn GetHighBandwidthBias(&self, bandwidth: DataRate) -> f64 {
        if bandwidth.IsFinite() {
            return self.AdjustBiasFactor(
                self.average_reported_loss_ratio,
                self.config.higher_bandwidth_bias_factor,
            ) * bandwidth.kbps_float()
                + self.AdjustBiasFactor(
                    self.average_reported_loss_ratio,
                    self.config.higher_log_bandwidth_bias_factor,
                ) * (1.0 + bandwidth.kbps_float()).ln();
        }
        0.0
    }
    fn GetObjective(&self, channel_parameters: &ChannelParameters) -> f64 {
        let mut objective: f64 = 0.0;

        let high_bandwidth_bias: f64 =
            self.GetHighBandwidthBias(channel_parameters.loss_limited_bandwidth);

        for observation in &self.observations {
            if !observation.IsInitialized() {
                continue;
            }

            let loss_probability: f64 = GetLossProbability(
                channel_parameters.inherent_loss,
                channel_parameters.loss_limited_bandwidth,
                observation.sending_rate,
            );

            let id: usize = observation.id.try_into().unwrap();
            let temporal_weight: f64 = self.temporal_weights[(self.num_observations - 1) - id];
            if self.config.use_byte_loss_rate {
                objective += temporal_weight
                    * ((ToKiloBytes(observation.lost_size) * loss_probability.ln())
                        + (ToKiloBytes(observation.size - observation.lost_size)
                            * (1.0 - loss_probability).ln()));
                objective += temporal_weight * high_bandwidth_bias * ToKiloBytes(observation.size);
            } else {
                objective += temporal_weight
                    * ((observation.num_lost_packets as f64 * (loss_probability).ln())
                        + (observation.num_received_packets as f64
                            * (1.0 - loss_probability).ln()));
                objective += temporal_weight * high_bandwidth_bias * observation.num_packets as f64;
            }
        }

        objective
    }
    fn GetSendingRate(&self, instantaneous_sending_rate: DataRate) -> DataRate {
        if self.num_observations <= 0 {
            return instantaneous_sending_rate;
        }

        let most_recent_observation_idx: usize =
            (self.num_observations - 1) % self.config.observation_window_size;
        let most_recent_observation = &self.observations[most_recent_observation_idx];
        let sending_rate_previous_observation: DataRate = most_recent_observation.sending_rate;

        self.config.sending_rate_smoothing_factor * sending_rate_previous_observation
            + (1.0 - self.config.sending_rate_smoothing_factor) * instantaneous_sending_rate
    }
    fn GetInstantUpperBound(&self) -> DataRate {
        self.cached_instant_upper_bound.unwrap_or(self.max_bitrate)
    }
    fn CalculateInstantUpperBound(&mut self) {
        let mut instant_limit: DataRate = self.max_bitrate;
        if self.average_reported_loss_ratio > self.config.instant_upper_bound_loss_offset {
            instant_limit = self.config.instant_upper_bound_bandwidth_balance
                / (self.average_reported_loss_ratio - self.config.instant_upper_bound_loss_offset);
        }

        self.cached_instant_upper_bound = Some(instant_limit);
    }
    fn GetInstantLowerBound(&self) -> DataRate {
        self.cached_instant_lower_bound.unwrap_or(DataRate::Zero())
    }
    fn CalculateInstantLowerBound(&mut self) {
        let mut instance_lower_bound: DataRate = DataRate::Zero();

        if let Some(acknowledged_bitrate) = self.acknowledged_bitrate {
            if self.config.lower_bound_by_acked_rate_factor > 0.0 {
                instance_lower_bound =
                    self.config.lower_bound_by_acked_rate_factor * acknowledged_bitrate;
            }
        }

        if self.min_bitrate.IsFinite() {
            instance_lower_bound = std::cmp::max(instance_lower_bound, self.min_bitrate);
        }
        self.cached_instant_lower_bound = Some(instance_lower_bound);
    }

    fn NewtonsMethodUpdate(&self, channel_parameters: &mut ChannelParameters) {
        if self.num_observations <= 0 {
            return;
        }

        for i in 0..self.config.newton_iterations {
            let derivatives: Derivatives = self.GetDerivatives(channel_parameters);
            channel_parameters.inherent_loss -=
                self.config.newton_step_size * derivatives.first / derivatives.second;
            channel_parameters.inherent_loss = self.GetFeasibleInherentLoss(channel_parameters);
        }
    }

    // Returns false if no observation was created.
    fn PushBackObservation(&mut self, packet_results: &[PacketResult]) -> bool {
        if packet_results.is_empty() {
            return false;
        }

        self.partial_observation.num_packets += packet_results.len();
        let mut last_send_time: Timestamp = Timestamp::MinusInfinity();
        let mut first_send_time: Timestamp = Timestamp::PlusInfinity();
        for packet in packet_results {
            if packet.IsReceived() {
                self.partial_observation
                    .lost_packets
                    .remove(&packet.sent_packet.sequence_number);
            } else {
                self.partial_observation
                    .lost_packets
                    .insert(packet.sent_packet.sequence_number, packet.sent_packet.size);
            }
            self.partial_observation.size += packet.sent_packet.size;
            last_send_time = std::cmp::max(last_send_time, packet.sent_packet.send_time);
            first_send_time = std::cmp::min(first_send_time, packet.sent_packet.send_time);
        }

        // This is the first packet report we have received.
        if !self.last_send_time_most_recent_observation.IsFinite() {
            self.last_send_time_most_recent_observation = first_send_time;
        }

        let observation_duration: TimeDelta =
            last_send_time - self.last_send_time_most_recent_observation;
        // Too small to be meaningful.
        // To consider: what if it is too long?, i.e. we did not receive any packets
        // for a long time, then all the packets we received are too old.
        if observation_duration <= TimeDelta::Zero()
            || observation_duration < self.config.observation_duration_lower_bound
        {
            return false;
        }

        self.last_send_time_most_recent_observation = last_send_time;

        let mut observation = Observation::default();
        observation.num_packets = self.partial_observation.num_packets;
        observation.num_lost_packets = self.partial_observation.lost_packets.len();
        observation.num_received_packets = observation.num_packets - observation.num_lost_packets;
        observation.sending_rate =
            self.GetSendingRate(self.partial_observation.size / observation_duration);
        for (key, packet_size) in &self.partial_observation.lost_packets {
            observation.lost_size += *packet_size;
        }
        observation.size = self.partial_observation.size;
        self.num_observations += 1;
        observation.id = self.num_observations as _;
        self.observations[self.num_observations % self.config.observation_window_size] =
            observation;

        self.partial_observation = PartialObservation::default();
        self.UpdateAverageReportedLossRatio();
        self.CalculateInstantUpperBound();
        true
    }
    fn IsEstimateIncreasingWhenLossLimited(
        &mut self,
        old_estimate: DataRate,
        new_estimate: DataRate,
    ) -> bool {
        (old_estimate < new_estimate
            || (old_estimate == new_estimate
                && (self.loss_based_result.state == LossBasedState::Increasing
                    || self.loss_based_result.state == LossBasedState::IncreaseUsingPadding)))
            && self.IsInLossLimitedState()
    }
    fn IsInLossLimitedState(&self) -> bool {
        self.loss_based_result.state != LossBasedState::DelayBasedEstimate
    }
    fn CanKeepIncreasingState(&self, estimate: DataRate) -> bool {
        if self.config.padding_duration == TimeDelta::Zero()
            || self.loss_based_result.state != LossBasedState::IncreaseUsingPadding
        {
            return true;
        }

        // Keep using the IncreaseUsingPadding if either the state has been
        // IncreaseUsingPadding for less than PaddingDuration or the estimate
        // increases.
        self.last_padding_info.padding_timestamp + self.config.padding_duration
            >= self.last_send_time_most_recent_observation
            || self.last_padding_info.padding_rate < estimate
    }
    fn GetMedianSendingRate(&self) -> DataRate {
        let mut sending_rates: Vec<DataRate> = Vec::new();
        for observation in &self.observations {
            if !observation.IsInitialized()
                || !observation.sending_rate.IsFinite()
                || observation.sending_rate.IsZero()
            {
                continue;
            }
            sending_rates.push(observation.sending_rate);
        }
        if sending_rates.is_empty() {
            return DataRate::Zero();
        }
        sending_rates.sort();
        if sending_rates.len() % 2 == 0 {
            return (sending_rates[sending_rates.len() / 2 - 1]
                + sending_rates[sending_rates.len() / 2])
                / 2;
        }
        sending_rates[sending_rates.len() / 2]
    }
}

fn ToKiloBytes(datasize: DataSize) -> f64 {
    datasize.bytes_float() / 1000.0
}

fn GetLossProbability(
    mut inherent_loss: f64,
    loss_limited_bandwidth: DataRate,
    sending_rate: DataRate,
) -> f64 {
    if !(0.0..=1.0).contains(&inherent_loss) {
        tracing::warn!("The inherent loss must be in [0,1]: {}", inherent_loss);
        inherent_loss = inherent_loss.max(0.0).min(1.0);
    }
    if !sending_rate.IsFinite() {
        tracing::warn!("The sending rate must be finite: {:?}", sending_rate);
    }
    if !loss_limited_bandwidth.IsFinite() {
        tracing::warn!(
            "The loss limited bandwidth must be finite: {:?}",
            loss_limited_bandwidth
        );
    }

    let mut loss_probability: f64 = inherent_loss;
    if sending_rate.IsFinite()
        && loss_limited_bandwidth.IsFinite()
        && (sending_rate > loss_limited_bandwidth)
    {
        loss_probability +=
            (1.0 - inherent_loss) * (sending_rate - loss_limited_bandwidth) / sending_rate;
    }
    loss_probability.max(1.0e-6).min(1.0 - 1.0e-6)
}
