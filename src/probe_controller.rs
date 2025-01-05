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

// NOTE: These have been renamed to match the field trials.
// WebRTC-Bwe-ProbingConfiguration
#[derive(Debug, Clone)]
pub struct ProbeControllerConfig {
    // These parameters configure the initial probes. First we send one or two
    // probes of sizes p1 * self.start_bitrate and p2 * self.start_bitrate.
    // Then whenever we get a bitrate estimate of at least further_probe_threshold
    // times the size of the last sent probe we'll send another one of size
    // step_size times the new estimate.
    pub p1: f64,
    pub p2: f64,
    pub step_size: f64,
    pub further_probe_threshold: f64,
    pub abort_further: bool,
    // Duration of time from the first initial probe where repeated initial probes
    // are sent if repeated initial probing is enabled.
    pub initial_probing: TimeDelta,
    // The minimum probing duration of an individual probe during
    // the repeated_initial_probing_time_period.
    pub initial_probe_duration: TimeDelta,
    // Delta time between sent bursts of packets in a probe during
    // the repeated_initial_probing_time_period.
    pub initial_min_probe_delta: TimeDelta,
    // Configures how often we send ALR probes and how big they are.
    pub alr_interval: TimeDelta,
    pub alr_scale: f64,
    // Configures how often we send probes if NetworkStateEstimate is available.
    pub network_state_interval: TimeDelta,
    // Periodically probe as long as the ratio between current estimate and
    // NetworkStateEstimate is lower then this.
    pub est_lower_than_network_ratio: f64,
    pub est_lower_than_network_interval: TimeDelta,
    pub network_state_scale: f64,
    // Overrides min_probe_duration if network_state_estimate_probing_interval
    // is set and a network state estimate is known and equal or higher than the
    // probe target.
    pub network_state_probe_duration: TimeDelta,
    // Overrides min_probe_delta if network_state_estimate_probing_interval
    // is set and a network state estimate is known and equal or higher than the
    // probe target.
    pub network_state_min_probe_delta: TimeDelta,

    // Configures the probes emitted by changed to the allocated bitrate.
    pub probe_max_allocation: bool,
    pub alloc_p1: f64,
    pub alloc_p2: f64,
    pub alloc_current_bwe_limit: f64,

    // The minimum number probing packets used.
    pub min_probe_packets_sent: i32,
    // The minimum probing duration.
    pub min_probe_duration: TimeDelta,
    // Delta time between sent bursts of packets in a probe.
    pub min_probe_delta: TimeDelta,
    pub loss_limited_scale: f64,
    // Don't send a probe if min(estimate, network state estimate) is larger than
    // this fraction of the set max or max allocated bitrate.
    pub skip_if_est_larger_than_fraction_of_max: f64,
    // Scale factor of the max allocated bitrate. Used when deciding if a probe
    // can be skiped due to that the estimate is already high enough.
    pub skip_max_allocated_scale: f64,
}

impl Default for ProbeControllerConfig {
    fn default() -> Self {
        Self {
            p1: 3.0,
            p2: 6.0,
            step_size: 2.0,
            further_probe_threshold: 0.7,
            abort_further: false,
            initial_probing: TimeDelta::Seconds(5),
            initial_probe_duration: TimeDelta::Millis(100),
            initial_min_probe_delta: TimeDelta::Millis(20),
            alr_interval: TimeDelta::Seconds(5),
            alr_scale: 2.0,
            network_state_interval: TimeDelta::PlusInfinity(),
            est_lower_than_network_ratio: 0.0,
            est_lower_than_network_interval: TimeDelta::Seconds(3),
            network_state_scale: 1.0,
            network_state_probe_duration: TimeDelta::Millis(15),
            network_state_min_probe_delta: TimeDelta::Millis(20),
            probe_max_allocation: true,
            alloc_p1: 1.0,
            alloc_p2: 2.0,
            alloc_current_bwe_limit: 2.0,
            min_probe_packets_sent: 5,
            min_probe_duration: TimeDelta::Millis(15),
            min_probe_delta: TimeDelta::Millis(2),
            loss_limited_scale: 1.5,
            skip_if_est_larger_than_fraction_of_max: 0.0,
            skip_max_allocated_scale: 1.0,
        }
    }
}

// Reason that bandwidth estimate is limited. Bandwidth estimate can be limited
// by either delay based bwe, or loss based bwe when it increases/decreases the
// estimate.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BandwidthLimitedCause {
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
        let mut this = Self {
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
        };
        this.Reset(Timestamp::Zero());
        this
    }
}

impl ProbeController {
    // the measured results back.
    const MaxWaitingTimeForProbingResult: TimeDelta = TimeDelta::Seconds(1);

    // Default probing bitrate limit. Applied only when the application didn't
    // specify max bitrate.
    const DefaultMaxProbingBitrate: DataRate = DataRate::KilobitsPerSec(5000);

    // If the bitrate drops to a factor `kBitrateDropThreshold` or lower
    // and we recover within `kBitrateDropTimeoutMs`, then we'll send
    // a probe at a fraction `kProbeFractionAfterDrop` of the original bitrate.
    const BitrateDropThreshold: f64 = 0.66;
    const BitrateDropTimeout: TimeDelta = TimeDelta::Seconds(5);
    const ProbeFractionAfterDrop: f64 = 0.85;

    // Timeout for probing after leaving ALR. If the bitrate drops significantly,
    // (as determined by the delay based estimator) and we leave ALR, then we will
    // send a probe if we recover within `kLeftAlrTimeoutMs` ms.
    const AlrEndedTimeout: TimeDelta = TimeDelta::Seconds(3);

    // The expected uncertainty of probe result (as a fraction of the target probe
    // This is a limit on how often probing can be done when there is a BW
    // drop detected in ALR.
    const MinTimeBetweenAlrProbes: TimeDelta = TimeDelta::Seconds(5);

    // bitrate). Used to avoid probing if the probe bitrate is close to our current
    // estimate.
    const ProbeUncertainty: f64 = 0.05;

    pub fn new(config: ProbeControllerConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn SetBitrates(
        &mut self,
        min_bitrate: DataRate,
        start_bitrate: DataRate,
        max_bitrate: DataRate,
        at_time: Timestamp,
    ) -> Vec<ProbeClusterConfig> {
        if start_bitrate > DataRate::Zero() {
            self.start_bitrate = start_bitrate;
            self.estimated_bitrate = start_bitrate;
        } else if self.start_bitrate.IsZero() {
            self.start_bitrate = min_bitrate;
        }

        // The reason we use the variable `old_max_bitrate_pbs` is because we
        // need to set `max_bitrate_` before we call InitiateProbing.
        let old_max_bitrate: DataRate = self.max_bitrate;
        self.max_bitrate = if max_bitrate.IsFinite() {
            max_bitrate
        } else {
            Self::DefaultMaxProbingBitrate
        };

        match self.state {
            State::Init => {
                if self.network_available {
                    return self.InitiateExponentialProbing(at_time);
                }
            }
            State::WaitingForProbingResult => (),
            State::ProbingComplete => {
                // If the new max bitrate is higher than both the old max bitrate and the
                // estimate then initiate probing.
                if !self.estimated_bitrate.IsZero()
                    && old_max_bitrate < self.max_bitrate
                    && self.estimated_bitrate < self.max_bitrate
                {
                    return self.InitiateProbing(at_time, &[self.max_bitrate], false);
                }
            }
        }
        vec![]
    }

    // The total bitrate, as opposed to the max bitrate, is the sum of the
    // configured bitrates for all active streams.

    pub fn OnMaxTotalAllocatedBitrate(
        &mut self,
        max_total_allocated_bitrate: DataRate,
        at_time: Timestamp,
    ) -> Vec<ProbeClusterConfig> {
        let in_alr: bool = self.alr_start_time.is_some();
        let allow_allocation_probe: bool = in_alr;
        if self.config.probe_max_allocation
            && matches!(self.state, State::ProbingComplete)
            && max_total_allocated_bitrate != self.max_total_allocated_bitrate
            && self.estimated_bitrate < self.max_bitrate
            && self.estimated_bitrate < max_total_allocated_bitrate
            && allow_allocation_probe
        {
            self.max_total_allocated_bitrate = max_total_allocated_bitrate;

            if self.config.alloc_p1 == 0.0 {
                return vec![];
            }

            let mut first_probe_rate: DataRate = max_total_allocated_bitrate * self.config.alloc_p1;
            let current_bwe_limit: DataRate =
                self.config.alloc_current_bwe_limit * self.estimated_bitrate;
            let mut limited_by_current_bwe: bool = current_bwe_limit < first_probe_rate;
            if limited_by_current_bwe {
                first_probe_rate = current_bwe_limit;
            }

            let mut probes: Vec<DataRate> = vec![first_probe_rate];
            if self.config.alloc_p2 > 0.0 && !limited_by_current_bwe {
                let mut second_probe_rate: DataRate =
                    max_total_allocated_bitrate * self.config.alloc_p2;
                limited_by_current_bwe = current_bwe_limit < second_probe_rate;
                if limited_by_current_bwe {
                    second_probe_rate = current_bwe_limit;
                }
                if second_probe_rate > first_probe_rate {
                    probes.push(second_probe_rate);
                }
            }

            let allow_further_probing: bool = limited_by_current_bwe;

            return self.InitiateProbing(at_time, &probes, allow_further_probing);
        }
        if !max_total_allocated_bitrate.IsZero() {
            self.last_allowed_repeated_initial_probe = at_time;
        }

        self.max_total_allocated_bitrate = max_total_allocated_bitrate;
        vec![]
    }

    pub fn OnNetworkAvailability(&mut self, msg: NetworkAvailability) -> Vec<ProbeClusterConfig> {
        self.network_available = msg.network_available;

        if !self.network_available && matches!(self.state, State::WaitingForProbingResult) {
            self.state = State::ProbingComplete;
            self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
        }

        if self.network_available
            && matches!(self.state, State::Init)
            && !self.start_bitrate.IsZero()
        {
            return self.InitiateExponentialProbing(msg.at_time);
        }
        vec![]
    }

    pub fn SetEstimatedBitrate(
        &mut self,
        bitrate: DataRate,
        bandwidth_limited_cause: BandwidthLimitedCause,
        at_time: Timestamp,
    ) -> Vec<ProbeClusterConfig> {
        self.bandwidth_limited_cause = bandwidth_limited_cause;
        if bitrate < Self::BitrateDropThreshold * self.estimated_bitrate {
            self.time_of_last_large_drop = at_time;
            self.bitrate_before_last_large_drop = self.estimated_bitrate;
        }
        self.estimated_bitrate = bitrate;

        if matches!(self.state, State::WaitingForProbingResult) {
            // Continue probing if probing results indicate channel has greater
            // capacity unless we already reached the needed bitrate.
            if self.config.abort_further
                && (bitrate > self.max_bitrate
                    || (!self.max_total_allocated_bitrate.IsZero()
                        && bitrate > 2 * self.max_total_allocated_bitrate))
            {
                // No need to continue probing.
                self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
            }
            let network_state_estimate_probe_further_limit: DataRate = match self.network_estimate {
                Some(network_estimate)
                    if self.config.network_state_interval.IsFinite()
                        && network_estimate.link_capacity_upper.IsFinite() =>
                {
                    network_estimate.link_capacity_upper * self.config.further_probe_threshold
                }
                _ => DataRate::PlusInfinity(),
            };
            tracing::info!(
                "Measured bitrate: {:?} Minimum to probe further: {:?} upper limit: {:?}",
                bitrate,
                self.min_bitrate_to_probe_further,
                network_state_estimate_probe_further_limit
            );

            if bitrate > self.min_bitrate_to_probe_further
                && bitrate <= network_state_estimate_probe_further_limit
            {
                return self.InitiateProbing(at_time, &[self.config.step_size * bitrate], true);
            }
        }
        vec![]
    }

    pub fn EnablePeriodicAlrProbing(&mut self, enable: bool) {
        self.enable_periodic_alr_probing = enable;
    }

    // Probes are sent periodically every 1s during the first 5s after the network
    // becomes available or until OnMaxTotalAllocatedBitrate is invoked with a
    // none zero max_total_allocated_bitrate (there are active streams being
    // sent.) Probe rate is up to max configured bitrate configured via
    // SetBitrates.
    pub fn EnableRepeatedInitialProbing(&mut self, enable: bool) {
        self.repeated_initial_probing_enabled = enable;
    }

    pub fn SetAlrStartTimeMs(&mut self, alr_start_time: Option<i64>) {
        self.alr_start_time = alr_start_time.map(Timestamp::Millis);
    }
    pub fn SetAlrEndedTimeMs(&mut self, alr_end_time: i64) {
        self.alr_end_time = Some(Timestamp::Millis(alr_end_time));
    }

    pub fn RequestProbe(&mut self, at_time: Timestamp) -> Vec<ProbeClusterConfig> {
        // Called once we have returned to normal state after a large drop in
        // estimated bandwidth. The current response is to initiate a single probe
        // session (if not already probing) at the previous bitrate.
        //
        // If the probe session fails, the assumption is that this drop was a
        // real one from a competing flow or a network change.
        let in_alr: bool = self.alr_start_time.is_some();
        let alr_ended_recently: bool = match self.alr_end_time {
            Some(alr_end_time) => at_time - alr_end_time < Self::AlrEndedTimeout,
            _ => false,
        };

        if (in_alr || alr_ended_recently || self.in_rapid_recovery_experiment)
            && matches!(self.state, State::ProbingComplete)
        {
            let suggested_probe: DataRate =
                Self::ProbeFractionAfterDrop * self.bitrate_before_last_large_drop;
            let min_expected_probe_result: DataRate =
                (1.0 - Self::ProbeUncertainty) * suggested_probe;
            let time_since_drop: TimeDelta = at_time - self.time_of_last_large_drop;
            let time_since_probe: TimeDelta = at_time - self.last_bwe_drop_probing_time;
            if min_expected_probe_result > self.estimated_bitrate
                && time_since_drop < Self::BitrateDropTimeout
                && time_since_probe > Self::MinTimeBetweenAlrProbes
            {
                tracing::info!("Detected big bandwidth drop, start probing.");
                // Track how often we probe in response to bandwidth drop in ALR.
                self.last_bwe_drop_probing_time = at_time;
                return self.InitiateProbing(at_time, &[suggested_probe], false);
            }
        }
        vec![]
    }

    pub fn SetNetworkStateEstimate(&mut self, estimate: NetworkStateEstimate) {
        self.network_estimate = Some(estimate);
    }

    // Resets the ProbeController to a state equivalent to as if it was just
    // created EXCEPT for configuration settings like
    // `enable_periodic_alr_probing_` `network_available_` and
    // `max_total_allocated_bitrate_`.
    pub fn Reset(&mut self, at_time: Timestamp) {
        self.bandwidth_limited_cause = BandwidthLimitedCause::DelayBasedLimited;
        self.state = State::Init;
        self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
        self.time_last_probing_initiated = Timestamp::Zero();
        self.estimated_bitrate = DataRate::Zero();
        self.network_estimate = None;
        self.start_bitrate = DataRate::Zero();
        self.max_bitrate = Self::DefaultMaxProbingBitrate;
        let now: Timestamp = at_time;
        self.last_bwe_drop_probing_time = now;
        self.alr_end_time.take();
        self.time_of_last_large_drop = now;
        self.bitrate_before_last_large_drop = DataRate::Zero();
    }

    pub fn Process(&mut self, at_time: Timestamp) -> Vec<ProbeClusterConfig> {
        if at_time - self.time_last_probing_initiated > Self::MaxWaitingTimeForProbingResult
            && matches!(self.state, State::WaitingForProbingResult)
        {
            tracing::info!("kWaitingForProbingResult: timeout");
            self.UpdateState(State::ProbingComplete);
        }
        if self.estimated_bitrate.IsZero() || !matches!(self.state, State::ProbingComplete) {
            return vec![];
        }
        if self.TimeForNextRepeatedInitialProbe(at_time) {
            return self.InitiateProbing(at_time, &[self.estimated_bitrate * self.config.p1], true);
        }
        if self.TimeForAlrProbe(at_time) || self.TimeForNetworkStateProbe(at_time) {
            return self.InitiateProbing(
                at_time,
                &[self.estimated_bitrate * self.config.alr_scale],
                true,
            );
        }
        vec![]
    }

    fn UpdateState(&mut self, new_state: State) {
        match new_state {
            State::Init => {
                self.state = State::Init;
            }
            State::WaitingForProbingResult => {
                self.state = State::WaitingForProbingResult;
            }
            State::ProbingComplete => {
                self.state = State::ProbingComplete;
                self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
            }
        }
    }
    fn InitiateExponentialProbing(&mut self, at_time: Timestamp) -> Vec<ProbeClusterConfig> {
        assert!(self.network_available);
        assert!(matches!(self.state, State::Init));
        assert!(self.start_bitrate > DataRate::Zero());

        // When probing at 1.8 Mbps ( 6x 300), this represents a threshold of
        // 1.2 Mbps to continue probing.
        let mut probes: Vec<DataRate> = vec![self.config.p1 * self.start_bitrate];
        if self.config.p2 > 0.0 {
            probes.push(self.config.p2 * self.start_bitrate);
        }

        if self.repeated_initial_probing_enabled && self.max_total_allocated_bitrate.IsZero() {
            self.last_allowed_repeated_initial_probe = at_time + self.config.initial_probing;
            tracing::info!(
                "Repeated initial probing enabled, last allowed probe: {:?} now: {:?}",
                self.last_allowed_repeated_initial_probe,
                at_time
            );
        }

        self.InitiateProbing(at_time, &probes, true)
    }
    fn InitiateProbing(
        &mut self,
        now: Timestamp,
        bitrates_to_probe: &[DataRate],
        mut probe_further: bool,
    ) -> Vec<ProbeClusterConfig> {
        if self.config.skip_if_est_larger_than_fraction_of_max > 0.0 {
            let network_estimate: DataRate = self
                .network_estimate
                .map(|x| x.link_capacity_upper)
                .unwrap_or(DataRate::Zero());
            let max_probe_rate: DataRate = if self.max_total_allocated_bitrate.IsZero() {
                self.max_bitrate
            } else {
                std::cmp::min(
                    self.config.skip_max_allocated_scale * self.max_total_allocated_bitrate,
                    self.max_bitrate,
                )
            };
            if std::cmp::min(network_estimate, self.estimated_bitrate)
                > self.config.skip_if_est_larger_than_fraction_of_max * max_probe_rate
            {
                self.UpdateState(State::ProbingComplete);
                return vec![];
            }
        }

        let mut max_probe_bitrate: DataRate = self.max_bitrate;
        if self.max_total_allocated_bitrate > DataRate::Zero() {
            // If a max allocated bitrate has been configured, allow probing up to 2x
            // that rate. This allows some overhead to account for bursty streams,
            // which otherwise would have to ramp up when the overshoot is already in
            // progress.
            // It also avoids minor quality reduction caused by probes often being
            // received at slightly less than the target probe bitrate.
            max_probe_bitrate =
                std::cmp::min(max_probe_bitrate, self.max_total_allocated_bitrate * 2);
        }

        match self.bandwidth_limited_cause {
            BandwidthLimitedCause::RttBasedBackOffHighRtt
            | BandwidthLimitedCause::DelayBasedLimitedDelayIncreased
            | BandwidthLimitedCause::LossLimitedBwe => {
                tracing::info!(
                    "Not sending probe in bandwidth limited state. {:?}",
                    self.bandwidth_limited_cause
                );
                return vec![];
            }
            BandwidthLimitedCause::LossLimitedBweIncreasing => {
                max_probe_bitrate = std::cmp::min(
                    max_probe_bitrate,
                    self.estimated_bitrate * self.config.loss_limited_scale,
                );
            }
            _ => (),
        };

        match self.network_estimate {
            Some(network_estimate) if network_estimate.link_capacity_upper.IsFinite() => {
                if self.config.network_state_interval.IsFinite() {
                    if network_estimate.link_capacity_upper.IsZero() {
                        tracing::info!("Not sending probe, Network state estimate is zero");
                        return vec![];
                    }
                    max_probe_bitrate = std::cmp::min(
                        max_probe_bitrate,
                        std::cmp::max(
                            self.estimated_bitrate,
                            network_estimate.link_capacity_upper * self.config.network_state_scale,
                        ),
                    );
                }
            }
            _ => (),
        };

        let mut pending_probes: Vec<ProbeClusterConfig> = vec![];
        for mut bitrate in bitrates_to_probe.iter().cloned() {
            assert!(!bitrate.IsZero());
            if bitrate >= max_probe_bitrate {
                bitrate = max_probe_bitrate;
                probe_further = false;
            }
            pending_probes.push(self.CreateProbeClusterConfig(now, bitrate));
        }
        self.time_last_probing_initiated = now;
        if probe_further {
            self.UpdateState(State::WaitingForProbingResult);
            // Dont expect probe results to be larger than a fraction of the actual
            // probe rate.
            self.min_bitrate_to_probe_further = pending_probes.last().unwrap().target_data_rate
                * self.config.further_probe_threshold;
        } else {
            self.UpdateState(State::ProbingComplete);
        }
        pending_probes
    }
    fn TimeForAlrProbe(&self, at_time: Timestamp) -> bool {
        if let Some(alr_start_time) = self.alr_start_time {
            if self.enable_periodic_alr_probing {
                let next_probe_time: Timestamp =
                    std::cmp::max(alr_start_time, self.time_last_probing_initiated)
                        + self.config.alr_interval;
                return at_time >= next_probe_time;
            }
        }
        false
    }
    fn TimeForNetworkStateProbe(&self, at_time: Timestamp) -> bool {
        let network_estimate = match self.network_estimate.as_ref() {
            Some(network_estimate) if !network_estimate.link_capacity_upper.IsInfinite() => {
                network_estimate
            }
            _ => return false,
        };

        let probe_due_to_low_estimate: bool = self.bandwidth_limited_cause
            == BandwidthLimitedCause::DelayBasedLimited
            && self.estimated_bitrate
                < self.config.est_lower_than_network_ratio * network_estimate.link_capacity_upper;
        if probe_due_to_low_estimate && self.config.est_lower_than_network_interval.IsFinite() {
            let next_probe_time: Timestamp =
                self.time_last_probing_initiated + self.config.est_lower_than_network_interval;
            return at_time >= next_probe_time;
        }

        let periodic_probe: bool = self.estimated_bitrate < network_estimate.link_capacity_upper;
        if periodic_probe && self.config.network_state_interval.IsFinite() {
            let next_probe_time: Timestamp =
                self.time_last_probing_initiated + self.config.network_state_interval;
            return at_time >= next_probe_time;
        }

        false
    }
    fn TimeForNextRepeatedInitialProbe(&self, at_time: Timestamp) -> bool {
        if !matches!(self.state, State::WaitingForProbingResult)
            && self.last_allowed_repeated_initial_probe > at_time
        {
            let next_probe_time: Timestamp =
                self.time_last_probing_initiated + Self::MaxWaitingTimeForProbingResult;
            if at_time >= next_probe_time {
                return true;
            }
        }
        false
    }
    fn CreateProbeClusterConfig(
        &mut self,
        at_time: Timestamp,
        bitrate: DataRate,
    ) -> ProbeClusterConfig {
        let mut config = ProbeClusterConfig::default();
        config.at_time = at_time;
        config.target_data_rate = bitrate;

        match self.network_estimate {
            Some(network_estimate)
                if network_estimate.link_capacity_upper.IsFinite()
                    && network_estimate.link_capacity_upper >= bitrate =>
            {
                config.target_duration = self.config.network_state_probe_duration;
                config.min_probe_delta = self.config.network_state_min_probe_delta;
            }
            _ => {
                if at_time < self.last_allowed_repeated_initial_probe {
                    config.target_duration = self.config.initial_probe_duration;
                    config.min_probe_delta = self.config.initial_min_probe_delta;
                } else {
                    config.target_duration = self.config.min_probe_duration;
                    config.min_probe_delta = self.config.min_probe_delta;
                }
            }
        };

        config.target_probe_count = self.config.min_probe_packets_sent;
        config.id = self.next_probe_cluster_id;
        self.next_probe_cluster_id += 1;
        config
    }
}

#[cfg(test)]
mod test {
    use super::*;
    const MinBitrate: DataRate = DataRate::BitsPerSec(100);
    const StartBitrate: DataRate = DataRate::BitsPerSec(300);
    const MaxBitrate: DataRate = DataRate::BitsPerSec(10000);

    const ExponentialProbingTimeout: TimeDelta = TimeDelta::Seconds(5);

    const AlrProbeInterval: TimeDelta = TimeDelta::Seconds(5);
    const AlrEndedTimeout: TimeDelta = TimeDelta::Seconds(3);
    const BitrateDropTimeout: TimeDelta = TimeDelta::Seconds(5);

    #[test]
    fn InitiatesProbingAfterSetBitrates() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(probes.len() >= 2);
    }

    #[test]
    fn InitiatesProbingWhenNetworkAvailable() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });

        let probes: Vec<ProbeClusterConfig> =
            probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(probes.is_empty());
        let probes = probe_controller.OnNetworkAvailability(NetworkAvailability {
            network_available: true,
            ..Default::default()
        });
        assert!(probes.len() >= 2);
    }

    #[test]
    fn SetsDefaultTargetDurationAndTargetProbeCount() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes: Vec<ProbeClusterConfig> =
            probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(probes.len() >= 2);

        assert_eq!(probes[0].target_duration, TimeDelta::Millis(15));
        assert_eq!(probes[0].target_probe_count, 5);
    }

    #[test]
    fn FieldTrialsOverrideDefaultTargetDurationAndTargetProbeCount() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            min_probe_packets_sent: 2,
            min_probe_duration: TimeDelta::Millis(123),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes: Vec<ProbeClusterConfig> =
            probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(probes.len() >= 2);

        assert_eq!(probes[0].target_duration, TimeDelta::Millis(123));
        assert_eq!(probes[0].target_probe_count, 2);
    }

    #[test]
    fn ProbeOnlyWhenNetworkIsUp() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        let probes = probe_controller.OnNetworkAvailability(NetworkAvailability {
            at_time: clock,
            network_available: false,
        });
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(probes.is_empty());
        let probes = probe_controller.OnNetworkAvailability(NetworkAvailability {
            at_time: clock,
            network_available: true,
        });
        assert!(probes.len() >= 2);
    }

    #[test]
    fn CanConfigureInitialProbeRateFactor() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            p1: 2.0,
            p2: 3.0,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].target_data_rate, StartBitrate * 2);
        assert_eq!(probes[1].target_data_rate, StartBitrate * 3);
    }

    #[test]
    fn DisableSecondInitialProbeIfRateFactorZero() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            p1: 2.0,
            p2: 0.0,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, StartBitrate * 2);
    }

    #[test]
    fn InitiatesProbingOnMaxBitrateIncrease() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        // Long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        probe_controller.Process(clock);
        let probes = probe_controller.SetBitrates(
            MinBitrate,
            StartBitrate,
            MaxBitrate + DataRate::BitsPerSec(100),
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps(), MaxBitrate.bps() + 100);
    }

    #[test]
    fn ProbesOnMaxAllocatedBitrateIncreaseOnlyWhenInAlr() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate - DataRate::BitsPerSec(1),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        // Wait long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        // Probe when in alr.
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        let probes = probe_controller
            .OnMaxTotalAllocatedBitrate(MaxBitrate + DataRate::BitsPerSec(1), clock);
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].target_data_rate, MaxBitrate);

        // Do not probe when not in alr.
        probe_controller.SetAlrStartTimeMs(None);
        let probes = probe_controller
            .OnMaxTotalAllocatedBitrate(MaxBitrate + DataRate::BitsPerSec(2), clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn ProbesOnMaxAllocatedBitrateLimitedByCurrentBwe() {
        let mut clock: Timestamp = Timestamp::Zero();

        assert!(MaxBitrate > 1.5 * StartBitrate);
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        // Wait long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        // Probe when in alr.
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        let probes = probe_controller.OnMaxTotalAllocatedBitrate(MaxBitrate, clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, 2.0 * StartBitrate);

        // Continue probing if probe succeeds.
        let probes = probe_controller.SetEstimatedBitrate(
            1.5 * StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert!(probes[0].target_data_rate > 1.5 * StartBitrate);
    }

    #[test]
    fn CanDisableProbingOnMaxTotalAllocatedBitrateIncrease() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            probe_max_allocation: false,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate - DataRate::BitsPerSec(1),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));

        // Do no probe, since probe_max_allocation:false.
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        let probes = probe_controller
            .OnMaxTotalAllocatedBitrate(MaxBitrate + DataRate::BitsPerSec(1), clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn InitiatesProbingOnMaxBitrateIncreaseAtMaxBitrate() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        // Long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        probe_controller.Process(clock);
        probe_controller.SetEstimatedBitrate(
            MaxBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        let probes = probe_controller.SetBitrates(
            MinBitrate,
            StartBitrate,
            MaxBitrate + DataRate::BitsPerSec(100),
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0].target_data_rate,
            MaxBitrate + DataRate::BitsPerSec(100)
        );
    }

    #[test]
    fn TestExponentialProbing() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);

        // Repeated probe should only be sent when estimated bitrate climbs above
        // 0.7 * 6 * StartBitrate = 1260.
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1000),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1800),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps(), 2 * 1800);
    }

    #[test]
    fn ExponentialProbingStopIfMaxBitrateLow() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            abort_further: true,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Repeated probe normally is sent when estimated bitrate climbs above
        // 0.7 * 6 * StartBitrate = 1260. But since max bitrate is low, expect
        // exponential probing to stop.
        let probes = probe_controller.SetBitrates(
            MinBitrate,
            StartBitrate,
            /*max_bitrate=*/ StartBitrate,
            clock,
        );
        assert!(probes.is_empty());

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1800),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn ExponentialProbingStopIfMaxAllocatedBitrateLow() {
        let clock = Timestamp::Zero();
        let mut probe_controller: ProbeController = ProbeController::new(ProbeControllerConfig {
            abort_further: true,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Repeated probe normally is sent when estimated bitrate climbs above
        // 0.7 * 6 * StartBitrate = 1260. But since allocated bitrate i slow, expect
        // exponential probing to stop.
        let probes = probe_controller.OnMaxTotalAllocatedBitrate(StartBitrate, clock);
        assert!(probes.is_empty());

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1800),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn InitialProbingToLowMaxAllocatedbitrate() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Repeated probe is sent when estimated bitrate climbs above
        // 0.7 * 6 * StartBitrate = 1260.
        let probes = probe_controller.OnMaxTotalAllocatedBitrate(StartBitrate, clock);
        assert!(probes.is_empty());

        // If the inital probe result is received, a new probe is sent at 2x the
        // needed max bitrate.
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1800),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps(), 2 * StartBitrate.bps());
    }

    #[test]
    fn InitialProbingTimeout() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());
        // Advance far enough to cause a time out in waiting for probing result.
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1800),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn RepeatedInitialProbingSendsNewProbeAfterTimeout() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        probe_controller.EnableRepeatedInitialProbing(true);
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());
        let start_time: Timestamp = clock;
        let mut last_probe_time: Timestamp = clock;
        while clock < start_time + TimeDelta::Seconds(5) {
            clock += TimeDelta::Millis(100);
            let probes = probe_controller.Process(clock);
            if !probes.is_empty() {
                // Expect a probe every second.
                assert_eq!(clock - last_probe_time, TimeDelta::SecondsFloat(1.1));
                assert_eq!(probes[0].min_probe_delta, TimeDelta::Millis(20));
                assert_eq!(probes[0].target_duration, TimeDelta::Millis(100));
                last_probe_time = clock;
            } else {
                assert!(clock - last_probe_time < TimeDelta::SecondsFloat(1.1));
            }
        }
        clock += TimeDelta::Seconds(1);
        // After 5s, repeated initial probing stops.
        assert!(probe_controller.Process(clock).is_empty());
    }

    #[test]
    fn RepeatedInitialProbingStopIfMaxAllocatedBitrateSet() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        probe_controller.EnableRepeatedInitialProbing(true);
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        let probes = probe_controller.OnMaxTotalAllocatedBitrate(MinBitrate, clock);
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn RequestProbeInAlr() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(probes.len() >= 2);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(250),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        let probes = probe_controller.RequestProbe(clock);

        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps_float(), 0.85 * 500.0);
    }

    #[test]
    fn RequestProbeWhenAlrEndedRecently() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 2);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        probe_controller.SetAlrStartTimeMs(None);
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(250),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        probe_controller.SetAlrEndedTimeMs(clock.ms());
        clock += AlrEndedTimeout - TimeDelta::Millis(1);
        let probes = probe_controller.RequestProbe(clock);

        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps_float(), 0.85 * 500.0);
    }

    #[test]
    fn RequestProbeWhenAlrNotEndedRecently() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 2);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        probe_controller.SetAlrStartTimeMs(None);
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(250),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        probe_controller.SetAlrEndedTimeMs(clock.ms());
        clock += AlrEndedTimeout + TimeDelta::Millis(1);
        let probes = probe_controller.RequestProbe(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn RequestProbeWhenBweDropNotRecent() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 2);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(250),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        clock += BitrateDropTimeout + TimeDelta::Millis(1);
        let probes = probe_controller.RequestProbe(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn PeriodicProbing() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 2);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        let start_time: Timestamp = clock;

        // Expect the controller to send a new probe after 5s has passed.
        probe_controller.SetAlrStartTimeMs(Some(start_time.ms()));
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps(), 1000);

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        // The following probe should be sent at 10s into ALR.
        probe_controller.SetAlrStartTimeMs(Some(start_time.ms()));
        clock += TimeDelta::Seconds(4);
        let probes = probe_controller.Process(clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        probe_controller.SetAlrStartTimeMs(Some(start_time.ms()));
        clock += TimeDelta::Seconds(1);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn PeriodicProbingAfterReset() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let alr_start_time: Timestamp = clock;

        probe_controller.SetAlrStartTimeMs(Some(alr_start_time.ms()));
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        probe_controller.Reset(clock);

        clock += TimeDelta::Seconds(10);
        let probes = probe_controller.Process(clock);
        // Since bitrates are not yet set, no probe is sent event though we are in ALR
        // mode.
        assert!(probes.is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert_eq!(probes.len(), 2);

        // Make sure we use `kStartBitrateBps` as the estimated bitrate
        // until SetEstimatedBitrate is called with an updated estimate.
        clock += TimeDelta::Seconds(10);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, StartBitrate * 2);
    }

    #[test]
    fn NoProbesWhenTransportIsNotWritable() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        probe_controller.EnablePeriodicAlrProbing(true);

        let probes: Vec<ProbeClusterConfig> =
            probe_controller.OnNetworkAvailability(NetworkAvailability {
                network_available: false,
                ..Default::default()
            });
        assert!(probes.is_empty());
        assert!(probe_controller
            .SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock)
            .is_empty());
        clock += TimeDelta::Seconds(10);
        assert!(probe_controller.Process(clock).is_empty());

        // Controller is reset after a network route change.
        // But, a probe should not be sent since the transport is not writable.
        // Transport is not writable until after DTLS negotiation completes.
        // However, the bitrate constraints may change.
        probe_controller.Reset(clock);
        assert!(probe_controller
            .SetBitrates(2 * MinBitrate, 2 * StartBitrate, 2 * MaxBitrate, clock)
            .is_empty());
        clock += TimeDelta::Seconds(10);
        assert!(probe_controller.Process(clock).is_empty());
    }

    #[test]
    fn TestExponentialProbingOverflow() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        const MbpsMultiplier: DataRate = DataRate::KilobitsPerSec(1000);
        let probes = probe_controller.SetBitrates(
            MinBitrate,
            10 * MbpsMultiplier,
            100 * MbpsMultiplier,
            clock,
        );
        // Verify that probe bitrate is capped at the specified max bitrate.
        let probes = probe_controller.SetEstimatedBitrate(
            60 * MbpsMultiplier,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, 100 * MbpsMultiplier);
        // Verify that repeated probes aren't sent.
        let probes = probe_controller.SetEstimatedBitrate(
            100 * MbpsMultiplier,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn TestAllocatedBitrateCap() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        const MbpsMultiplier: DataRate = DataRate::KilobitsPerSec(1000);
        const MaxBitrate: DataRate = DataRate::KilobitsPerSec(100 * 1000);
        let probes =
            probe_controller.SetBitrates(MinBitrate, 10 * MbpsMultiplier, MaxBitrate, clock);

        // Configure ALR for periodic probing.
        probe_controller.EnablePeriodicAlrProbing(true);
        let alr_start_time: Timestamp = clock;
        probe_controller.SetAlrStartTimeMs(Some(alr_start_time.ms()));

        let estimated_bitrate: DataRate = MaxBitrate / 10;
        let probes = probe_controller.SetEstimatedBitrate(
            estimated_bitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        // Set a max allocated bitrate below the current estimate.
        let max_allocated: DataRate = estimated_bitrate - 1 * MbpsMultiplier;
        let probes = probe_controller.OnMaxTotalAllocatedBitrate(max_allocated, clock);
        assert!(probes.is_empty()); // No probe since lower than current max.

        // Probes such as ALR capped at 2x the max allocation limit.
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, 2 * max_allocated);

        // Remove allocation limit.
        assert!(probe_controller
            .OnMaxTotalAllocatedBitrate(DataRate::Zero(), clock)
            .is_empty());
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, estimated_bitrate * 2);
    }

    #[test]
    fn ConfigurableProbingFieldTrial() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            p1: 2.0,
            p2: 5.0,
            step_size: 3.0,
            further_probe_threshold: 0.8,
            alloc_p1: 2.0,
            alloc_current_bwe_limit: 1000.0,
            alloc_p2: 5.0,
            min_probe_packets_sent: 2,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(
            MinBitrate,
            StartBitrate,
            DataRate::KilobitsPerSec(5000),
            clock,
        );
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].target_data_rate.bps(), 600);
        assert_eq!(probes[0].target_probe_count, 2);
        assert_eq!(probes[1].target_data_rate.bps(), 1500);
        assert_eq!(probes[1].target_probe_count, 2);

        // Repeated probe should only be sent when estimated bitrate climbs above
        // 0.8 * 5 * StartBitrateBps = 1200.
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1100),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert_eq!(probes.len(), 0);

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(1250),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps(), 3 * 1250);

        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);

        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        let probes =
            probe_controller.OnMaxTotalAllocatedBitrate(DataRate::KilobitsPerSec(200), clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate.bps(), 400_000);
    }

    #[test]
    fn LimitAlrProbeWhenLossBasedBweLimited() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        // Expect the controller to send a new probe after 5s has passed.
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::LossLimitedBweIncreasing,
            clock,
        );
        clock += TimeDelta::Seconds(6);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, 1.5 * DataRate::BitsPerSec(500));

        let probes = probe_controller.SetEstimatedBitrate(
            1.5 * DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        clock += TimeDelta::Seconds(6);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert!(probes[0].target_data_rate > 1.5 * 1.5 * DataRate::BitsPerSec(500));
    }

    #[test]
    fn PeriodicProbeAtUpperNetworkStateEstimate() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(5000),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        // Expect the controller to send a new probe after 5s has passed.
        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = DataRate::KilobitsPerSec(6);
        probe_controller.SetNetworkStateEstimate(state_estimate);

        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0].target_data_rate,
            state_estimate.link_capacity_upper
        );
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0].target_data_rate,
            state_estimate.link_capacity_upper
        );
    }

    #[test]
    fn LimitProbeAtUpperNetworkStateEstimateIfLossBasedLimited() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        // Expect the controller to send a new probe after 5s has passed.
        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = DataRate::BitsPerSec(700);
        probe_controller.SetNetworkStateEstimate(state_estimate);
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);

        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::BitsPerSec(500),
            BandwidthLimitedCause::LossLimitedBweIncreasing,
            clock,
        );
        // Expect the controller to send a new probe after 5s has passed.
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert_eq!(probes[0].target_data_rate, DataRate::BitsPerSec(700));
    }

    #[test]
    fn AlrProbesLimitedByNetworkStateEstimate() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::KilobitsPerSec(6),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));

        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, MaxBitrate);

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = DataRate::BitsPerSec(8000);
        probe_controller.SetNetworkStateEstimate(state_estimate);
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0].target_data_rate,
            state_estimate.link_capacity_upper
        );
    }

    #[test]
    fn CanSetLongerProbeDurationAfterNetworkStateEstimate() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            network_state_probe_duration: TimeDelta::Millis(100),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            DataRate::KilobitsPerSec(5),
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(!probes.is_empty());
        assert!(probes[0].target_duration < TimeDelta::Millis(100));

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = DataRate::KilobitsPerSec(6);
        probe_controller.SetNetworkStateEstimate(state_estimate);
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_duration, TimeDelta::Millis(100));
    }

    #[test]
    fn ProbeInAlrIfLossBasedIncreasing() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::LossLimitedBweIncreasing,
            clock,
        );

        // Wait long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        // Probe when in alr.
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].target_data_rate, 1.5 * StartBitrate);
    }

    #[test]
    fn NotProbeWhenInAlrIfLossBasedDecreases() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::LossLimitedBwe,
            clock,
        );

        // Wait long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        // Not probe in alr when loss based estimate decreases.
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn NotProbeIfLossBasedIncreasingOutsideAlr() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::LossLimitedBweIncreasing,
            clock,
        );

        // Wait long enough to time out exponential probing.
        clock += ExponentialProbingTimeout;
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        probe_controller.SetAlrStartTimeMs(None);
        clock += AlrProbeInterval + TimeDelta::Millis(1);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn ProbeFurtherWhenLossBasedIsSameAsDelayBasedEstimate() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = 5 * StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());

        let probe_target_rate: DataRate = probes[0].target_data_rate;
        assert!(probe_target_rate < state_estimate.link_capacity_upper);
        // Expect that more probes are sent if BWE is the same as delay based
        // estimate.
        let probes = probe_controller.SetEstimatedBitrate(
            probe_target_rate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(!probes.is_empty());
        assert_eq!(probes[0].target_data_rate, 2 * probe_target_rate);
    }

    #[test]
    fn ProbeIfEstimateLowerThanNetworkStateEstimate() {
        // Periodic probe every 1 second if estimate is lower than 50% of the
        // NetworkStateEstimate.
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            est_lower_than_network_interval: TimeDelta::Seconds(1),
            est_lower_than_network_ratio: 0.5,
            // limit_probe_target_rate_to_loss_bwe: true,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        state_estimate.link_capacity_upper = StartBitrate * 3;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        let probes = probe_controller.Process(clock);
        assert_eq!(probes.len(), 1);
        assert!(probes[0].target_data_rate > StartBitrate);

        // If network state not increased, send another probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());

        // Stop probing if estimate increase. We might probe further here though.
        let probes = probe_controller.SetEstimatedBitrate(
            2 * StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        // No more periodic probes.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn DontProbeFurtherWhenLossLimited() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = 3 * StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert!(probes[0].target_data_rate < state_estimate.link_capacity_upper);
        // Expect that no more probes are sent immediately if BWE is loss limited.
        let probes = probe_controller.SetEstimatedBitrate(
            probes[0].target_data_rate,
            BandwidthLimitedCause::LossLimitedBwe,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn ProbeFurtherWhenDelayBasedLimited() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = 3 * StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert!(probes[0].target_data_rate < state_estimate.link_capacity_upper);
        // Since the probe was successfull, expect to continue probing.
        let probes = probe_controller.SetEstimatedBitrate(
            probes[0].target_data_rate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(!probes.is_empty());
        assert_eq!(
            probes[0].target_data_rate,
            state_estimate.link_capacity_upper
        );
    }

    #[test]
    fn ProbeAfterTimeoutIfNetworkStateEstimateIncreaseAfterProbeSent() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            est_lower_than_network_interval: TimeDelta::Seconds(3),
            est_lower_than_network_ratio: 0.7,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());
        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = 1.2 * probes[0].target_data_rate / 2.0;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        // No immediate further probing since probe result is low.
        let probes = probe_controller.SetEstimatedBitrate(
            probes[0].target_data_rate / 2,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert!(probes[0].target_data_rate <= state_estimate.link_capacity_upper);
        // If the network state estimate increase, even before the probe result,
        // expect a new probe after `est_lower_than_network_interval` timeout.
        state_estimate.link_capacity_upper = 3 * StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        let probes = probe_controller.SetEstimatedBitrate(
            probes[0].target_data_rate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        clock += TimeDelta::Seconds(3);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());

        // But no more probes if estimate is close to the link capacity.
        let probes = probe_controller.SetEstimatedBitrate(
            state_estimate.link_capacity_upper * 0.9,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
    }

    #[test]
    fn SkipProbeFurtherIfAlreadyProbedToMaxRate() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(2),
            skip_if_est_larger_than_fraction_of_max: 0.9,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        probe_controller.SetNetworkStateEstimate(NetworkStateEstimate {
            link_capacity_upper: 2 * MaxBitrate,
            ..Default::default()
        });

        // Attempt to probe up to max rate.
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate * 0.8,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(!probes.is_empty());
        assert_eq!(probes[0].target_data_rate, MaxBitrate);

        // If the probe result arrives, dont expect a new probe immediately since we
        // already tried to probe at the max rate.
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate * 0.8,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        clock += TimeDelta::Millis(1000);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
        // But when enough time has passed, expect a new probe.
        clock += TimeDelta::Millis(1000);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
    }

    #[test]
    fn MaxAllocatedBitrateNotReset() {
        let clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        let probes = probe_controller.OnMaxTotalAllocatedBitrate(StartBitrate / 4, clock);
        probe_controller.Reset(clock);

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());
        assert_eq!(probes[0].target_data_rate, StartBitrate / 4 * 2);
    }

    #[test]
    fn SkipAlrProbeIfEstimateLargerThanMaxProbe() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            skip_if_est_larger_than_fraction_of_max: 0.9,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        clock += TimeDelta::Seconds(10);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        // But if the max rate increase, A new probe is sent.
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, 2 * MaxBitrate, clock);
        assert!(!probes.is_empty());
    }

    #[test]
    fn SkipAlrProbeIfEstimateLargerThanFractionOfMaxAllocated() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            skip_if_est_larger_than_fraction_of_max: 1.0,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.EnablePeriodicAlrProbing(true);
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate / 2,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );

        clock += TimeDelta::Seconds(10);
        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        let probes = probe_controller.OnMaxTotalAllocatedBitrate(MaxBitrate / 2, clock);
        // No probes since total allocated is not higher than the current estimate.
        assert!(probes.is_empty());
        clock += TimeDelta::Seconds(2);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        // But if the max allocated increase, A new probe is sent.
        let probes = probe_controller
            .OnMaxTotalAllocatedBitrate(MaxBitrate / 2 + DataRate::BitsPerSec(1), clock);
        assert!(!probes.is_empty());
    }

    #[test]
    fn SkipNetworkStateProbeIfEstimateLargerThanMaxProbe() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(2),
            skip_if_est_larger_than_fraction_of_max: 0.9,
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        probe_controller.SetNetworkStateEstimate(NetworkStateEstimate {
            link_capacity_upper: 2 * MaxBitrate,
            ..Default::default()
        });
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        clock += TimeDelta::Seconds(10);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn SendsProbeIfNetworkStateEstimateLowerThanMaxProbe() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(2),
            skip_if_est_larger_than_fraction_of_max: 0.9,
            network_state_probe_duration: TimeDelta::Millis(100),
            network_state_min_probe_delta: TimeDelta::Millis(20),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());
        probe_controller.SetNetworkStateEstimate(NetworkStateEstimate {
            link_capacity_upper: 2 * MaxBitrate,
            ..Default::default()
        });
        let probes = probe_controller.SetEstimatedBitrate(
            MaxBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());

        // Need to wait at least two seconds before process can trigger a new probe.
        clock += TimeDelta::Millis(2100);

        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        assert!(probes.is_empty());
        probe_controller.SetNetworkStateEstimate(NetworkStateEstimate {
            link_capacity_upper: 2 * StartBitrate,
            ..Default::default()
        });
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert!(probes[0].target_data_rate <= 2 * StartBitrate);
        // Expect probe durations to be picked from field trial probe target is lower
        // or equal to the network state estimate.
        assert_eq!(probes[0].min_probe_delta, TimeDelta::Millis(20));
        assert_eq!(probes[0].target_duration, TimeDelta::Millis(100));
    }

    #[test]
    fn ProbeNotLimitedByNetworkStateEsimateIfLowerThantCurrent() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            network_state_interval: TimeDelta::Seconds(5),
            network_state_probe_duration: TimeDelta::Millis(100),
            network_state_min_probe_delta: TimeDelta::Millis(20),
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());
        probe_controller.EnablePeriodicAlrProbing(true);
        probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimited,
            clock,
        );
        probe_controller.SetNetworkStateEstimate(NetworkStateEstimate {
            link_capacity_upper: StartBitrate,
            ..Default::default()
        });
        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        probe_controller.SetAlrStartTimeMs(Some(clock.ms()));
        probe_controller.SetNetworkStateEstimate(NetworkStateEstimate {
            link_capacity_upper: StartBitrate / 2,
            ..Default::default()
        });
        clock += TimeDelta::Seconds(6);
        let probes = probe_controller.Process(clock);
        assert!(!probes.is_empty());
        assert_eq!(probes[0].target_data_rate, StartBitrate);
        // Expect probe durations to be default since network state estimate is lower
        // than the probe rate.
        assert_eq!(probes[0].min_probe_delta, TimeDelta::Millis(2));
        assert_eq!(probes[0].target_duration, TimeDelta::Millis(15));
    }

    #[test]
    fn DontProbeIfDelayIncreased() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = 3 * StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::DelayBasedLimitedDelayIncreased,
            clock,
        );
        assert!(probes.is_empty());

        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }

    #[test]
    fn DontProbeIfHighRtt() {
        let mut clock = Timestamp::Zero();
        let mut probe_controller = ProbeController::new(ProbeControllerConfig {
            ..Default::default()
        });
        assert!(probe_controller
            .OnNetworkAvailability(NetworkAvailability {
                network_available: true,
                ..Default::default()
            })
            .is_empty());

        let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate, MaxBitrate, clock);
        assert!(!probes.is_empty());

        // Need to wait at least one second before process can trigger a new probe.
        clock += TimeDelta::Millis(1100);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());

        let mut state_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        state_estimate.link_capacity_upper = 3 * StartBitrate;
        probe_controller.SetNetworkStateEstimate(state_estimate);
        let probes = probe_controller.SetEstimatedBitrate(
            StartBitrate,
            BandwidthLimitedCause::RttBasedBackOffHighRtt,
            clock,
        );
        assert!(probes.is_empty());

        clock += TimeDelta::Seconds(5);
        let probes = probe_controller.Process(clock);
        assert!(probes.is_empty());
    }
}
