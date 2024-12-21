/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::api::{
    transport::{NetworkAvailability, NetworkStateEstimate, ProbeClusterConfig},
    units::{DataRate, TimeDelta, Timestamp},
};

#[derive(Default)] // TODO
struct ProbeControllerConfig {
    // These parameters configure the initial probes. First we send one or two
    // probes of sizes p1 * self.start_bitrate and p2 * self.start_bitrate.
    // Then whenever we get a bitrate estimate of at least further_probe_threshold
    // times the size of the last sent probe we'll send another one of size
    // step_size times the new estimate.
    pub first_exponential_probe_scale: f64,
    pub second_exponential_probe_scale: Option<f64>,
    pub further_exponential_probe_scale: f64,
    pub further_probe_threshold: f64,
    pub abort_further_probe_if_max_lower_than_current: bool,
    // Duration of time from the first initial probe where repeated initial probes
    // are sent if repeated initial probing is enabled.
    pub repeated_initial_probing_time_period: TimeDelta,
    // The minimum probing duration of an individual probe during
    // the repeated_initial_probing_time_period.
    pub initial_probe_duration: TimeDelta,
    // Delta time between sent bursts of packets in a probe during
    // the repeated_initial_probing_time_period.
    pub initial_min_probe_delta: TimeDelta,
    // Configures how often we send ALR probes and how big they are.
    pub alr_probing_interval: TimeDelta,
    pub alr_probe_scale: f64,
    // Configures how often we send probes if NetworkStateEstimate is available.
    pub network_state_estimate_probing_interval: TimeDelta,
    // Periodically probe as long as the ratio between current estimate and
    // NetworkStateEstimate is lower then this.
    pub probe_if_estimate_lower_than_network_state_estimate_ratio: f64,
    pub estimate_lower_than_network_state_estimate_probing_interval: TimeDelta,
    pub network_state_probe_scale: f64,
    // Overrides min_probe_duration if network_state_estimate_probing_interval
    // is set and a network state estimate is known and equal or higher than the
    // probe target.
    pub network_state_probe_duration: TimeDelta,
    // Overrides min_probe_delta if network_state_estimate_probing_interval
    // is set and a network state estimate is known and equal or higher than the
    // probe target.
    pub network_state_min_probe_delta: TimeDelta,

    // Configures the probes emitted by changed to the allocated bitrate.
    pub probe_on_max_allocated_bitrate_change: bool,
    pub first_allocation_probe_scale: Option<f64>,
    pub second_allocation_probe_scale: Option<f64>,
    pub allocation_probe_limit_by_current_scale: f64,

    // The minimum number probing packets used.
    pub min_probe_packets_sent: isize,
    // The minimum probing duration.
    pub min_probe_duration: TimeDelta,
    // Delta time between sent bursts of packets in a probe.
    pub min_probe_delta: TimeDelta,
    pub loss_limited_probe_scale: f64,
    // Don't send a probe if min(estimate, network state estimate) is larger than
    // this fraction of the set max or max allocated bitrate.
    pub skip_if_estimate_larger_than_fraction_of_max: f64,
    // Scale factor of the max allocated bitrate. Used when deciding if a probe
    // can be skiped due to that the estimate is already high enough.
    pub skip_probe_max_allocated_scale: f64,
}

// Reason that bandwidth estimate is limited. Bandwidth estimate can be limited
// by either delay based bwe, or loss based bwe when it increases/decreases the
// estimate.
enum BandwidthLimitedCause {
    LossLimitedBweIncreasing = 0,
    LossLimitedBwe = 1,
    DelayBasedLimited = 2,
    DelayBasedLimitedDelayIncreased = 3,
    RttBasedBackOffHighRtt = 4,
}

enum State {
    // Initial state where no probing has been triggered yet.
    Init,
    // Waiting for probing results to continue further probing.
    WaitingForProbingResult,
    // Probing is complete.
    ProbingComplete,
}

// This class controls initiation of probing to estimate initial channel
// capacity. There is also support for probing during a session when max
// bitrate is adjusted by an application.
pub struct ProbeController {
    network_available: bool,
    repeated_initial_probing_enabled: bool,
    last_allowed_repeated_initial_probe: Timestamp,
    bandwidth_limited_cause: BandwidthLimitedCause,
    state: State,
    min_bitrate_to_probe_further: DataRate,
    time_last_probing_initiated: Timestamp,
    estimated_bitrate: DataRate,
    network_estimate: Option<NetworkStateEstimate>,
    start_bitrate: DataRate,
    max_bitrate: DataRate,
    last_bwe_drop_probing_time: Timestamp,
    alr_start_time: Option<Timestamp>,
    alr_end_time: Option<Timestamp>,
    enable_periodic_alr_probing: bool,
    time_of_last_large_drop: Timestamp,
    bitrate_before_last_large_drop: DataRate,
    max_total_allocated_bitrate: DataRate,

    in_rapid_recovery_experiment: bool,
    next_probe_cluster_id: i32,

    config: ProbeControllerConfig,
}

impl Default for ProbeController {
    fn default() -> Self {
        Self {
            network_available: false,
            state: State::Init,
            min_bitrate_to_probe_further: DataRate::PlusInfinity(),
            time_last_probing_initiated: Timestamp::MinusInfinity(),
            estimated_bitrate: DataRate::Zero(),
            network_estimate: None,
            start_bitrate: DataRate::Zero(),
            max_bitrate: DataRate::PlusInfinity(),
            last_bwe_drop_probing_time: Timestamp::Zero(),
            alr_start_time: None,
            alr_end_time: None,
            enable_periodic_alr_probing: false,
            time_of_last_large_drop: Timestamp::MinusInfinity(),
            bitrate_before_last_large_drop: DataRate::Zero(),
            max_total_allocated_bitrate: DataRate::Zero(),
            in_rapid_recovery_experiment: false,
            next_probe_cluster_id: 1,
            config: ProbeControllerConfig::default(),
            repeated_initial_probing_enabled: false,
            last_allowed_repeated_initial_probe: Timestamp::MinusInfinity(),
            bandwidth_limited_cause: BandwidthLimitedCause::DelayBasedLimited,
        }
    }
}

impl ProbeController {
    pub fn SetBitrates(
        &mut self,
        min_bitrate: DataRate,
        start_bitrate: DataRate,
        max_bitrate: DataRate,
        at_time: Timestamp,
    ) -> Vec<ProbeClusterConfig> {
        todo!();
    }

    // The total bitrate, as opposed to the max bitrate, is the sum of the
    // configured bitrates for all active streams.

    pub fn OnMaxTotalAllocatedBitrate(
        &mut self,
        max_total_allocated_bitrate: DataRate,
        at_time: Timestamp,
    ) -> Vec<ProbeClusterConfig> {
        todo!();
    }

    pub fn OnNetworkAvailability(&mut self, msg: NetworkAvailability) -> Vec<ProbeClusterConfig> {
        todo!();
    }

    pub fn SetEstimatedBitrate(
        &mut self,
        bitrate: DataRate,
        bandwidth_limited_cause: BandwidthLimitedCause,
        at_time: Timestamp,
    ) -> Vec<ProbeClusterConfig> {
        todo!()
    }

    pub fn EnablePeriodicAlrProbing(&mut self, enable: bool) {
        todo!();
    }

    // Probes are sent periodically every 1s during the first 5s after the network
    // becomes available or until OnMaxTotalAllocatedBitrate is invoked with a
    // none zero max_total_allocated_bitrate (there are active streams being
    // sent.) Probe rate is up to max configured bitrate configured via
    // SetBitrates.
    pub fn EnableRepeatedInitialProbing(&mut self, enable: bool) {
        todo!();
    }

    pub fn SetAlrStartTimeMs(&mut self, alr_start_time: Option<i64>) {
        todo!();
    }
    pub fn SetAlrEndedTimeMs(&mut self, alr_end_time: i64) {
        todo!();
    }

    pub fn RequestProbe(&mut self, at_time: Timestamp) -> Vec<ProbeClusterConfig> {
        todo!();
    }

    pub fn SetNetworkStateEstimate(&mut self, estimate: NetworkStateEstimate) {
        todo!();
    }

    // Resets the ProbeController to a state equivalent to as if it was just
    // created EXCEPT for configuration settings like
    // `enable_periodic_alr_probing_` `network_available_` and
    // `max_total_allocated_bitrate_`.
    pub fn Reset(&mut self, at_time: Timestamp) {
        todo!();
    }

    pub fn Process(&mut self, at_time: Timestamp) -> Vec<ProbeClusterConfig> {
        todo!();
    }

    fn UpdateState(&mut self, new_state: State) {
        todo!();
    }
    fn InitiateExponentialProbing(&mut self, at_time: Timestamp) -> Vec<ProbeClusterConfig> {
        todo!();
    }
    fn InitiateProbing(
        &mut self,
        now: Timestamp,
        bitrates_to_probe: Vec<DataRate>,
        probe_further: bool,
    ) -> Vec<ProbeClusterConfig> {
        todo!();
    }
    fn TimeForAlrProbe(&self, at_time: Timestamp) -> bool {
        todo!();
    }
    fn TimeForNetworkStateProbe(&self, at_time: Timestamp) -> bool {
        todo!();
    }
    fn TimeForNextRepeatedInitialProbe(&self, at_time: Timestamp) -> bool {
        todo!();
    }
    fn CreateProbeClusterConfig(at_time: Timestamp, bitrate: DataRate) -> ProbeClusterConfig {
        todo!();
    }
}
