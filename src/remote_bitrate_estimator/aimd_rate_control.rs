/*
 *  Copyright (c) 2014 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::{
    api::{
        transport::{BandwidthUsage, NetworkStateEstimate},
        units::{DataRate, DataSize, TimeDelta, Timestamp},
    },
    remote_bitrate_estimator::BitrateWindow,
    FieldTrials, LinkCapacityEstimator,
};

use super::{CongestionControllerMinBitrate, RateControlInput};

// WebRTC-BweBackOffFactor
#[derive(Debug, Clone)]
pub struct BweBackOffFactor {
    pub backoff_factor: f64, // Enabled-*
}

impl Default for BweBackOffFactor {
    fn default() -> Self {
        Self {
            backoff_factor: Self::DefaultBackoffFactor,
        }
    }
}

impl BweBackOffFactor {
    const DefaultBackoffFactor: f64 = 0.85;

    pub fn validate(&mut self) {
        if self.backoff_factor >= 1.0 {
            tracing::warn!("Back-off factor must be less than 1.");
        } else if self.backoff_factor <= 0.0 {
            tracing::warn!("Back-off factor must be greater than 0.");
        } else {
            return;
        }

        tracing::warn!("Failed to parse parameters for AimdRateControl experiment from field trial string. Using default.");
        self.backoff_factor = Self::DefaultBackoffFactor
    }
}

// WebRTC-Bwe-EstimateBoundedIncrease
#[derive(Debug, Clone)]
pub struct EstimateBoundedIncrease {
    pub disable_estimate_bounded_increase: bool,
    pub use_current_estimate_as_min_upper_bound: bool,
}

impl Default for EstimateBoundedIncrease {
    fn default() -> Self {
        Self {
            disable_estimate_bounded_increase: false,
            use_current_estimate_as_min_upper_bound: true,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum RateControlState {
    Hold,
    Increase,
    Decrease,
}

// A rate control implementation based on additive increases of
// bitrate when no over-use is detected and multiplicative decreases when
// over-uses are detected. When we think the available bandwidth has changes or
// is unknown, we will switch to a "slow-start mode" where we increase
// multiplicatively.
pub struct AimdRateControl {
    min_configured_bitrate: DataRate,
    max_configured_bitrate: DataRate,
    current_bitrate: DataRate,
    latest_estimated_throughput: DataRate,
    link_capacity: LinkCapacityEstimator,
    network_estimate: Option<NetworkStateEstimate>,
    rate_control_state: RateControlState,
    time_last_bitrate_change: Timestamp,
    time_last_bitrate_decrease: Timestamp,
    time_first_throughput_estimate: Timestamp,
    bitrate_is_initialized: bool,
    beta: f64,
    in_alr: bool,
    rtt: TimeDelta,
    send_side: bool,
    // Allow the delay based estimate to only increase as long as application
    // limited region (alr) is not detected.
    no_bitrate_increase_in_alr: bool,
    last_decrease: Option<DataRate>,

    // Field trials
    disable_estimate_bounded_increase: bool,
    use_current_estimate_as_min_upper_bound: bool,
}

impl Default for AimdRateControl {
    fn default() -> Self {
        let max_configured_bitrate = DataRate::KilobitsPerSec(30000);
        Self {
            min_configured_bitrate: CongestionControllerMinBitrate,
            max_configured_bitrate,
            current_bitrate: max_configured_bitrate,
            latest_estimated_throughput: max_configured_bitrate,
            link_capacity: LinkCapacityEstimator::new(),
            rate_control_state: RateControlState::Hold,
            time_last_bitrate_change: Timestamp::MinusInfinity(),
            time_last_bitrate_decrease: Timestamp::MinusInfinity(),
            time_first_throughput_estimate: Timestamp::MinusInfinity(),
            bitrate_is_initialized: false,
            beta: Self::DefaultBackoffFactor,
            in_alr: false,
            rtt: Self::DefaultRtt,
            send_side: false,
            no_bitrate_increase_in_alr: false,
            network_estimate: None,
            last_decrease: None,
            disable_estimate_bounded_increase: false,
            use_current_estimate_as_min_upper_bound: true,
        }
    }
}

impl AimdRateControl {
    const DefaultRtt: TimeDelta = TimeDelta::Millis(200);
    const DefaultBackoffFactor: f64 = 0.85;

    pub fn new(field_trials: &FieldTrials, send_side: bool) -> Self {
        Self {
            send_side,
            beta: field_trials.bwe_back_off_factor.backoff_factor,
            no_bitrate_increase_in_alr: field_trials.no_bitrate_increase_in_alr,
            use_current_estimate_as_min_upper_bound: field_trials
                .estimate_bounded_increase
                .use_current_estimate_as_min_upper_bound,
            disable_estimate_bounded_increase: field_trials
                .estimate_bounded_increase
                .disable_estimate_bounded_increase,
            ..Default::default()
        }
    }

    // Returns true if the target bitrate has been initialized. This happens
    // either if it has been explicitly set via SetStartBitrate/SetEstimate, or if
    // we have measured a throughput.
    pub fn ValidEstimate(&self) -> bool {
        self.bitrate_is_initialized
    }
    pub fn SetStartBitrate(&mut self, start_bitrate: DataRate) {
        self.current_bitrate = start_bitrate;
        self.latest_estimated_throughput = self.current_bitrate;
        self.bitrate_is_initialized = true;
    }
    pub fn SetMinBitrate(&mut self, min_bitrate: DataRate) {
        self.min_configured_bitrate = min_bitrate;
        self.current_bitrate = self.current_bitrate.max(min_bitrate);
    }
    pub fn GetFeedbackInterval(&self) -> TimeDelta {
        // Estimate how often we can send RTCP if we allocate up to 5% of bandwidth
        // to feedback.
        const RtcpSize: DataSize = DataSize::Bytes(80);
        let rtcp_bitrate: DataRate = self.current_bitrate * 0.05;
        let interval: TimeDelta = RtcpSize / rtcp_bitrate;
        const MinFeedbackInterval: TimeDelta = TimeDelta::Millis(200);
        const MaxFeedbackInterval: TimeDelta = TimeDelta::Millis(1000);
        interval.Clamped(MinFeedbackInterval, MaxFeedbackInterval)
    }

    // Returns true if the bitrate estimate hasn't been changed for more than
    // an RTT, or if the estimated_throughput is less than half of the current
    // estimate. Should be used to decide if we should reduce the rate further
    // when over-using.
    pub fn TimeToReduceFurther(
        &mut self,
        at_time: Timestamp,
        estimated_throughput: DataRate,
    ) -> bool {
        let bitrate_reduction_interval: TimeDelta = self
            .rtt
            .Clamped(TimeDelta::Millis(10), TimeDelta::Millis(200));
        if at_time - self.time_last_bitrate_change >= bitrate_reduction_interval {
            return true;
        }
        if self.ValidEstimate() {
            // TODO(terelius/holmer): Investigate consequences of increasing
            // the threshold to 0.95 * LatestEstimate().
            let threshold: DataRate = 0.5 * self.LatestEstimate();
            return estimated_throughput < threshold;
        }
        false
    }
    // As above. To be used if overusing before we have measured a throughput.
    pub fn InitialTimeToReduceFurther(&mut self, at_time: Timestamp) -> bool {
        self.ValidEstimate()
            && self
                .TimeToReduceFurther(at_time, self.LatestEstimate() / 2 - DataRate::BitsPerSec(1))
    }

    pub fn LatestEstimate(&self) -> DataRate {
        self.current_bitrate
    }
    pub fn SetRtt(&mut self, rtt: TimeDelta) {
        self.rtt = rtt;
    }
    pub fn Update(&mut self, input: RateControlInput, at_time: Timestamp) -> DataRate {
        // Set the initial bit rate value to what we're receiving the first half
        // second.
        // TODO(bugs.webrtc.org/9379): The comment above doesn't match to the code.
        if !self.bitrate_is_initialized {
            const InitializationTime: TimeDelta = TimeDelta::Seconds(5);
            assert!(BitrateWindow <= InitializationTime);

            if let Some(estimated_throughput) = input.estimated_throughput {
                if self.time_first_throughput_estimate.IsInfinite() {
                    self.time_first_throughput_estimate = at_time;
                } else if at_time - self.time_first_throughput_estimate > InitializationTime {
                    self.current_bitrate = estimated_throughput;
                    self.bitrate_is_initialized = true;
                }
            }
        }

        self.ChangeBitrate(input, at_time);
        self.current_bitrate
    }

    pub fn SetInApplicationLimitedRegion(&mut self, in_alr: bool) {
        self.in_alr = in_alr;
    }
    pub fn SetEstimate(&mut self, bitrate: DataRate, at_time: Timestamp) {
        self.bitrate_is_initialized = true;
        let prev_bitrate: DataRate = self.current_bitrate;
        self.current_bitrate = self.ClampBitrate(bitrate);
        self.time_last_bitrate_change = at_time;
        if self.current_bitrate < prev_bitrate {
            self.time_last_bitrate_decrease = at_time;
        }
    }
    pub fn SetNetworkStateEstimate(&mut self, estimate: Option<NetworkStateEstimate>) {
        self.network_estimate = estimate;
    }

    // Returns the increase rate when used bandwidth is near the link capacity.
    pub fn GetNearMaxIncreaseRateBpsPerSecond(&self) -> f64 {
        assert!(!self.current_bitrate.IsZero());
        const FrameInterval: TimeDelta = TimeDelta::SecondsFloat(1.0 / 30.0);
        let frame_size: DataSize = self.current_bitrate * FrameInterval;
        const PacketSize: DataSize = DataSize::Bytes(1200);
        let packets_per_frame: f64 = (frame_size / PacketSize).ceil();
        let avg_packet_size: DataSize = frame_size / packets_per_frame;

        // Approximate the over-use estimator delay to 100 ms.
        let mut response_time: TimeDelta = self.rtt + TimeDelta::Millis(100);

        response_time = response_time * 2;
        let increase_rate_bps_per_second: f64 = (avg_packet_size / response_time).bps_float();
        const MinIncreaseRateBpsPerSecond: f64 = 4000.0;
        increase_rate_bps_per_second.max(MinIncreaseRateBpsPerSecond)
    }
    // Returns the expected time between overuse signals (assuming steady state).
    pub fn GetExpectedBandwidthPeriod(&self) -> TimeDelta {
        const MinPeriod: TimeDelta = TimeDelta::Seconds(2);
        const DefaultPeriod: TimeDelta = TimeDelta::Seconds(3);
        const MaxPeriod: TimeDelta = TimeDelta::Seconds(50);

        let increase_rate_bps_per_second: f64 = self.GetNearMaxIncreaseRateBpsPerSecond();
        if let Some(last_decrease) = self.last_decrease {
            let time_to_recover_decrease_seconds: f64 =
                last_decrease.bps_float() / increase_rate_bps_per_second;
            let period: TimeDelta = TimeDelta::SecondsFloat(time_to_recover_decrease_seconds);
            period.Clamped(MinPeriod, MaxPeriod)
        } else {
            DefaultPeriod
        }
    }

    // Update the target bitrate based on, among other things, the current rate
    // control state, the current target bitrate and the estimated throughput.
    // When in the "increase" state the bitrate will be increased either
    // additively or multiplicatively depending on the rate control region. When
    // in the "decrease" state the bitrate will be decreased to slightly below the
    // current throughput. When in the "hold" state the bitrate will be kept
    // constant to allow built up queues to drain.
    fn ChangeBitrate(&mut self, input: RateControlInput, at_time: Timestamp) {
        let mut new_bitrate: Option<DataRate> = None;
        let estimated_throughput: DataRate = input
            .estimated_throughput
            .unwrap_or(self.latest_estimated_throughput);
        if let Some(estimated_throughput) = input.estimated_throughput {
            self.latest_estimated_throughput = estimated_throughput;
        }

        // An over-use should always trigger us to reduce the bitrate, even though
        // we have not yet established our first estimate. By acting on the over-use,
        // we will end up with a valid estimate.
        if !self.bitrate_is_initialized && input.bw_state != BandwidthUsage::Overusing {
            return;
        }

        self.ChangeState(input, at_time);

        match self.rate_control_state {
            RateControlState::Hold => (),
            RateControlState::Increase => {
                if estimated_throughput > self.link_capacity.UpperBound() {
                    self.link_capacity.Reset();
                }

                // We limit the new bitrate based on the troughput to avoid unlimited
                // bitrate increases. We allow a bit more lag at very low rates to not too
                // easily get stuck if the encoder produces uneven outputs.
                let mut increase_limit: DataRate =
                    1.5 * estimated_throughput + DataRate::KilobitsPerSec(10);
                if self.send_side && self.in_alr && self.no_bitrate_increase_in_alr {
                    // Do not increase the delay based estimate in alr since the estimator
                    // will not be able to get transport feedback necessary to detect if
                    // the new estimate is correct.
                    // If we have previously increased above the limit (for instance due to
                    // probing), we don't allow further changes.
                    increase_limit = self.current_bitrate;
                }

                if self.current_bitrate < increase_limit {
                    let increased_bitrate: DataRate = if self.link_capacity.has_estimate() {
                        // The link_capacity estimate is reset if the measured throughput
                        // is too far from the estimate. We can therefore assume that our
                        // target rate is reasonably close to link capacity and use additive
                        // increase.
                        let additive_increase: DataRate =
                            self.AdditiveRateIncrease(at_time, self.time_last_bitrate_change);
                        self.current_bitrate + additive_increase
                    } else {
                        // If we don't have an estimate of the link capacity, use faster ramp
                        // up to discover the capacity.
                        let multiplicative_increase: DataRate = self.MultiplicativeRateIncrease(
                            at_time,
                            self.time_last_bitrate_change,
                            self.current_bitrate,
                        );
                        self.current_bitrate + multiplicative_increase
                    };
                    new_bitrate = Some(increased_bitrate.min(increase_limit));
                }
                self.time_last_bitrate_change = at_time;
            }
            RateControlState::Decrease => {
                // Set bit rate to something slightly lower than the measured throughput
                // to get rid of any self-induced delay.
                let mut decreased_bitrate: DataRate = estimated_throughput * self.beta;
                if decreased_bitrate > DataRate::KilobitsPerSec(5) {
                    decreased_bitrate -= DataRate::KilobitsPerSec(5);
                }

                if decreased_bitrate > self.current_bitrate {
                    // TODO(terelius): The link_capacity estimate may be based on old
                    // throughput measurements. Relying on them may lead to unnecessary
                    // BWE drops.
                    if self.link_capacity.has_estimate() {
                        decreased_bitrate = self.beta * self.link_capacity.estimate();
                    }
                }
                // Avoid increasing the rate when over-using.
                if decreased_bitrate < self.current_bitrate {
                    new_bitrate = Some(decreased_bitrate);
                }

                if self.bitrate_is_initialized && estimated_throughput < self.current_bitrate {
                    if let Some(new_bitrate) = new_bitrate {
                        self.last_decrease = Some(self.current_bitrate - new_bitrate);
                    } else {
                        self.last_decrease = Some(DataRate::Zero());
                    }
                }
                if estimated_throughput < self.link_capacity.LowerBound() {
                    // The current throughput is far from the estimated link capacity. Clear
                    // the estimate to allow an immediate update in OnOveruseDetected.
                    self.link_capacity.Reset();
                }

                self.bitrate_is_initialized = true;
                self.link_capacity.OnOveruseDetected(estimated_throughput);
                // Stay on hold until the pipes are cleared.
                self.rate_control_state = RateControlState::Hold;
                self.time_last_bitrate_change = at_time;
                self.time_last_bitrate_decrease = at_time;
            }
        };

        self.current_bitrate = self.ClampBitrate(new_bitrate.unwrap_or(self.current_bitrate));
    }

    fn ClampBitrate(&self, mut new_bitrate: DataRate) -> DataRate {
        if let Some(network_estimate) = self.network_estimate.as_ref() {
            if !self.disable_estimate_bounded_increase
                && network_estimate.link_capacity_upper.IsFinite()
            {
                let upper_bound: DataRate = if self.use_current_estimate_as_min_upper_bound {
                    network_estimate
                        .link_capacity_upper
                        .max(self.current_bitrate)
                } else {
                    network_estimate.link_capacity_upper
                };
                new_bitrate = upper_bound.min(new_bitrate);
            }
            if network_estimate.link_capacity_lower.IsFinite() && new_bitrate < self.current_bitrate
            {
                new_bitrate = self
                    .current_bitrate
                    .min(new_bitrate.max(network_estimate.link_capacity_lower * self.beta));
            }
        }
        new_bitrate = new_bitrate.max(self.min_configured_bitrate);
        new_bitrate
    }

    fn MultiplicativeRateIncrease(
        &self,
        at_time: Timestamp,
        last_time: Timestamp,
        current_bitrate: DataRate,
    ) -> DataRate {
        let mut alpha: f64 = 1.08;
        if last_time.IsFinite() {
            let time_since_last_update = at_time - last_time;
            alpha = alpha.powf(time_since_last_update.seconds_float().min(1.0));
        }
        let multiplicative_increase: DataRate =
            (current_bitrate * (alpha - 1.0)).max(DataRate::BitsPerSec(1000));
        multiplicative_increase
    }
    fn AdditiveRateIncrease(&self, at_time: Timestamp, last_time: Timestamp) -> DataRate {
        let time_period_seconds: f64 = (at_time - last_time).seconds_float();
        let data_rate_increase_bps: f64 =
            self.GetNearMaxIncreaseRateBpsPerSecond() * time_period_seconds;
        DataRate::BitsPerSecFloat(data_rate_increase_bps)
    }
    fn ChangeState(&mut self, input: RateControlInput, at_time: Timestamp) {
        match input.bw_state {
            BandwidthUsage::Normal => {
                if self.rate_control_state == RateControlState::Hold {
                    self.time_last_bitrate_change = at_time;
                    self.rate_control_state = RateControlState::Increase;
                }
            }
            BandwidthUsage::Overusing => {
                if self.rate_control_state != RateControlState::Decrease {
                    self.rate_control_state = RateControlState::Decrease;
                }
            }
            BandwidthUsage::Underusing => {
                self.rate_control_state = RateControlState::Hold;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const InitialTime: Timestamp = Timestamp::Millis(123_456);

    const MinBwePeriod: TimeDelta = TimeDelta::Seconds(2);
    const DefaultPeriod: TimeDelta = TimeDelta::Seconds(3);
    const MaxBwePeriod: TimeDelta = TimeDelta::Seconds(50);

    // After an overuse, we back off to 85% to the received bitrate.
    const FractionAfterOveruse: f64 = 0.85;

    #[test]
    fn MinNearMaxIncreaseRateOnLowBandwith() {
        let mut aimd_rate_control = AimdRateControl::default();
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(30_000), InitialTime);
        assert_eq!(
            aimd_rate_control.GetNearMaxIncreaseRateBpsPerSecond(),
            4_000.0
        );
    }

    #[test]
    fn NearMaxIncreaseRateIs5kbpsOn90kbpsAnd200msRtt() {
        let mut aimd_rate_control = AimdRateControl::default();
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(90_000), InitialTime);
        assert_eq!(
            aimd_rate_control.GetNearMaxIncreaseRateBpsPerSecond(),
            5_000.0
        );
    }

    #[test]
    fn NearMaxIncreaseRateIs5kbpsOn60kbpsAnd100msRtt() {
        let mut aimd_rate_control = AimdRateControl::default();
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(60_000), InitialTime);
        aimd_rate_control.SetRtt(TimeDelta::Millis(100));
        assert_eq!(
            aimd_rate_control.GetNearMaxIncreaseRateBpsPerSecond(),
            5_000.0
        );
    }

    #[test]
    fn GetIncreaseRateAndBandwidthPeriod() {
        let mut aimd_rate_control = AimdRateControl::default();
        const Bitrate: DataRate = DataRate::BitsPerSec(300_000);
        aimd_rate_control.SetEstimate(Bitrate, InitialTime);
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Overusing, Some(Bitrate)),
            InitialTime,
        );
        assert!(
            (aimd_rate_control.GetNearMaxIncreaseRateBpsPerSecond() - 14_000.0).abs() <= 1_000.0,
        );
        assert_eq!(
            aimd_rate_control.GetExpectedBandwidthPeriod(),
            DefaultPeriod
        );
    }

    #[test]
    fn BweLimitedByAckedBitrate() {
        let mut aimd_rate_control = AimdRateControl::default();
        const AckedBitrate: DataRate = DataRate::BitsPerSec(10_000);
        let mut now: Timestamp = InitialTime;
        aimd_rate_control.SetEstimate(AckedBitrate, now);
        while now - InitialTime < TimeDelta::Seconds(20) {
            aimd_rate_control.Update(
                RateControlInput::new(BandwidthUsage::Normal, Some(AckedBitrate)),
                now,
            );
            now += TimeDelta::Millis(100);
        }
        assert!(aimd_rate_control.ValidEstimate());
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            1.5 * AckedBitrate + DataRate::BitsPerSec(10_000)
        );
    }

    #[test]
    fn BweNotLimitedByDecreasingAckedBitrate() {
        let mut aimd_rate_control = AimdRateControl::default();
        const AckedBitrate: DataRate = DataRate::BitsPerSec(10_000);
        let mut now: Timestamp = InitialTime;
        aimd_rate_control.SetEstimate(AckedBitrate, now);
        while now - InitialTime < TimeDelta::Seconds(20) {
            aimd_rate_control.Update(
                RateControlInput::new(BandwidthUsage::Normal, Some(AckedBitrate)),
                now,
            );
            now += TimeDelta::Millis(100);
        }
        assert!(aimd_rate_control.ValidEstimate());
        // If the acked bitrate decreases the BWE shouldn't be reduced to 1.5x
        // what's being acked, but also shouldn't get to increase more.
        let prev_estimate: DataRate = aimd_rate_control.LatestEstimate();
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Normal, Some(AckedBitrate / 2)),
            now,
        );
        let new_estimate: DataRate = aimd_rate_control.LatestEstimate();
        assert_eq!(new_estimate, prev_estimate);
        assert!(
            (new_estimate.bps() - (1.5f64 * AckedBitrate + DataRate::BitsPerSec(10_000)).bps())
                .abs()
                < 2_000
        );
    }

    #[test]
    fn DefaultPeriodUntilFirstOveruse() {
        let mut aimd_rate_control = AimdRateControl::default();
        aimd_rate_control.SetStartBitrate(DataRate::KilobitsPerSec(300));
        assert_eq!(
            aimd_rate_control.GetExpectedBandwidthPeriod(),
            DefaultPeriod
        );
        aimd_rate_control.Update(
            RateControlInput::new(
                BandwidthUsage::Overusing,
                Some(DataRate::KilobitsPerSec(280)),
            ),
            InitialTime,
        );
        assert_ne!(
            aimd_rate_control.GetExpectedBandwidthPeriod(),
            DefaultPeriod
        );
    }

    #[test]
    fn ExpectedPeriodAfterTypicalDrop() {
        let mut aimd_rate_control = AimdRateControl::default();
        // The rate increase at 216 kbps should be 12 kbps. If we drop from
        // 216 + 4*12 = 264 kbps, it should take 4 seconds to recover. Since we
        // back off to 0.85*acked_rate-5kbps, the acked bitrate needs to be 260
        // kbps to end up at 216 kbps.
        const InitialBitrate: DataRate = DataRate::BitsPerSec(264_000);
        const UpdatedBitrate: DataRate = DataRate::BitsPerSec(216_000);
        let AckedBitrate: DataRate =
            (UpdatedBitrate + DataRate::BitsPerSec(5_000)) / FractionAfterOveruse;
        let mut now: Timestamp = InitialTime;
        aimd_rate_control.SetEstimate(InitialBitrate, now);
        now += TimeDelta::Millis(100);
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Overusing, Some(AckedBitrate)),
            now,
        );
        assert_eq!(aimd_rate_control.LatestEstimate(), UpdatedBitrate);
        assert_eq!(
            aimd_rate_control.GetNearMaxIncreaseRateBpsPerSecond(),
            12_000.0
        );
        assert_eq!(
            aimd_rate_control.GetExpectedBandwidthPeriod(),
            TimeDelta::Seconds(4)
        );
    }

    #[test]
    fn BandwidthPeriodIsNotBelowMin() {
        let mut aimd_rate_control = AimdRateControl::default();
        const InitialBitrate: DataRate = DataRate::BitsPerSec(10_000);
        let mut now: Timestamp = InitialTime;
        aimd_rate_control.SetEstimate(InitialBitrate, now);
        now += TimeDelta::Millis(100);
        // Make a small (1.5 kbps) bitrate drop to 8.5 kbps.
        aimd_rate_control.Update(
            RateControlInput::new(
                BandwidthUsage::Overusing,
                Some(InitialBitrate - DataRate::BitsPerSec(1)),
            ),
            now,
        );
        assert_eq!(aimd_rate_control.GetExpectedBandwidthPeriod(), MinBwePeriod);
    }

    #[test]
    fn BandwidthPeriodIsNotAboveMaxNoSmoothingExp() {
        let mut aimd_rate_control = AimdRateControl::default();
        const InitialBitrate: DataRate = DataRate::BitsPerSec(10_010_000);
        let mut now: Timestamp = InitialTime;
        aimd_rate_control.SetEstimate(InitialBitrate, now);
        now += TimeDelta::Millis(100);
        // Make a large (10 Mbps) bitrate drop to 10 kbps.
        let AckedBitrate: DataRate = DataRate::BitsPerSec(10_000) / FractionAfterOveruse;
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Overusing, Some(AckedBitrate)),
            now,
        );
        assert_eq!(aimd_rate_control.GetExpectedBandwidthPeriod(), MaxBwePeriod);
    }

    #[test]
    fn SendingRateBoundedWhenThroughputNotEstimated() {
        let mut aimd_rate_control = AimdRateControl::default();
        const InitialBitrate: DataRate = DataRate::BitsPerSec(123_000);
        let mut now: Timestamp = InitialTime;
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Normal, Some(InitialBitrate)),
            now,
        );
        // AimdRateControl sets the initial bit rate to what it receives after
        // five seconds has passed.
        // TODO(bugs.webrtc.org/9379): The comment in the AimdRateControl does not
        // match the constant.
        const InitializationTime: TimeDelta = TimeDelta::Seconds(5);
        now += InitializationTime + TimeDelta::Millis(1);
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Normal, Some(InitialBitrate)),
            now,
        );
        for i in [0..100] {
            aimd_rate_control.Update(RateControlInput::new(BandwidthUsage::Normal, None), now);
            now += TimeDelta::Millis(100);
        }
        assert!(
            aimd_rate_control.LatestEstimate()
                <= InitialBitrate * 1.5 + DataRate::BitsPerSec(10_000)
        );
    }

    #[test]
    fn EstimateDoesNotIncreaseInAlr() {
        // When alr is detected, the delay based estimator is not allowed to increase
        // bwe since there will be no feedback from the network if the new estimate
        // is correct.
        // WebRTC-DontIncreaseDelayBasedBweInAlr/Enabled/
        let field_trials = FieldTrials {
            no_bitrate_increase_in_alr: true,
            ..Default::default()
        };
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        let mut now: Timestamp = InitialTime;
        const InitialBitrate: DataRate = DataRate::BitsPerSec(123_000);
        aimd_rate_control.SetEstimate(InitialBitrate, now);
        aimd_rate_control.SetInApplicationLimitedRegion(true);
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Normal, Some(InitialBitrate)),
            now,
        );
        assert_eq!(aimd_rate_control.LatestEstimate(), InitialBitrate);

        for i in 0..100 {
            aimd_rate_control.Update(RateControlInput::new(BandwidthUsage::Normal, None), now);
            now += TimeDelta::Millis(100);
        }
        assert_eq!(aimd_rate_control.LatestEstimate(), InitialBitrate);
    }

    #[test]
    fn SetEstimateIncreaseBweInAlr() {
        let field_trials = FieldTrials {
            no_bitrate_increase_in_alr: true,
            ..Default::default()
        };
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);

        const InitialBitrate: DataRate = DataRate::BitsPerSec(123_000);
        aimd_rate_control.SetEstimate(InitialBitrate, InitialTime);
        aimd_rate_control.SetInApplicationLimitedRegion(true);
        assert_eq!(aimd_rate_control.LatestEstimate(), InitialBitrate);
        aimd_rate_control.SetEstimate(2 * InitialBitrate, InitialTime);
        assert_eq!(aimd_rate_control.LatestEstimate(), 2 * InitialBitrate);
    }

    #[test]
    fn SetEstimateUpperLimitedByNetworkEstimate() {
        let field_trials = Default::default();
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(300_000), InitialTime);
        let mut network_estimate = NetworkStateEstimate::default();
        network_estimate.link_capacity_upper = DataRate::BitsPerSec(400_000);
        aimd_rate_control.SetNetworkStateEstimate(Some(network_estimate));
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(500_000), InitialTime);
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            network_estimate.link_capacity_upper
        );
    }

    #[test]
    fn SetEstimateDefaultUpperLimitedByCurrentBitrateIfNetworkEstimateIsLow() {
        let field_trials = Default::default();
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(500_000), InitialTime);
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            DataRate::BitsPerSec(500_000)
        );

        let mut network_estimate: NetworkStateEstimate = NetworkStateEstimate::default();
        network_estimate.link_capacity_upper = DataRate::BitsPerSec(300_000);
        aimd_rate_control.SetNetworkStateEstimate(Some(network_estimate));
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(700_000), InitialTime);
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            DataRate::BitsPerSec(500_000)
        );
    }

    #[test]
    fn SetEstimateNotUpperLimitedByCurrentBitrateIfNetworkEstimateIsLowIf() {
        // WebRTC-Bwe-EstimateBoundedIncrease/c_upper:false/
        let field_trials = FieldTrials {
            estimate_bounded_increase: EstimateBoundedIncrease {
                use_current_estimate_as_min_upper_bound: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);

        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(500_000), InitialTime);
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            DataRate::BitsPerSec(500_000)
        );

        let mut network_estimate = NetworkStateEstimate::default();
        network_estimate.link_capacity_upper = DataRate::BitsPerSec(300_000);
        aimd_rate_control.SetNetworkStateEstimate(Some(network_estimate));
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(700_000), InitialTime);
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            DataRate::BitsPerSec(300_000)
        );
    }

    #[test]
    fn SetEstimateLowerLimitedByNetworkEstimate() {
        let field_trials = Default::default();
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        let mut network_estimate = NetworkStateEstimate::default();
        network_estimate.link_capacity_lower = DataRate::BitsPerSec(400_000);
        aimd_rate_control.SetNetworkStateEstimate(Some(network_estimate));
        aimd_rate_control.SetEstimate(DataRate::BitsPerSec(100_000), InitialTime);
        // 0.85 is default backoff factor. (`beta_`)
        assert_eq!(
            aimd_rate_control.LatestEstimate(),
            network_estimate.link_capacity_lower * 0.85
        );
    }

    #[test]
    fn SetEstimateIgnoredIfLowerThanNetworkEstimateAndCurrent() {
        let field_trials = Default::default();
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        aimd_rate_control.SetEstimate(DataRate::KilobitsPerSec(200), InitialTime);
        assert_eq!(aimd_rate_control.LatestEstimate().kbps(), 200);
        let mut network_estimate = NetworkStateEstimate::default();
        network_estimate.link_capacity_lower = DataRate::KilobitsPerSec(400);
        aimd_rate_control.SetNetworkStateEstimate(Some(network_estimate));
        // Ignore the next SetEstimate, since the estimate is lower than 85% of
        // the network estimate.
        aimd_rate_control.SetEstimate(DataRate::KilobitsPerSec(100), InitialTime);
        assert_eq!(aimd_rate_control.LatestEstimate().kbps(), 200);
    }

    #[test]
    fn EstimateIncreaseWhileNotInAlr() {
        // Allow the estimate to increase as long as alr is not detected to ensure
        // tha BWE can not get stuck at a certain bitrate.
        // WebRTC-DontIncreaseDelayBasedBweInAlr/Enabled/
        let field_trials = FieldTrials {
            no_bitrate_increase_in_alr: true,
            ..Default::default()
        };
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        aimd_rate_control.no_bitrate_increase_in_alr = true;
        let mut now: Timestamp = InitialTime;
        const InitialBitrate: DataRate = DataRate::BitsPerSec(123_000);
        aimd_rate_control.SetEstimate(InitialBitrate, now);
        aimd_rate_control.SetInApplicationLimitedRegion(false);
        aimd_rate_control.Update(
            RateControlInput::new(BandwidthUsage::Normal, Some(InitialBitrate)),
            now,
        );
        for i in 0..100 {
            aimd_rate_control.Update(RateControlInput::new(BandwidthUsage::Normal, None), now);
            now += TimeDelta::Millis(100);
        }
        assert!(aimd_rate_control.LatestEstimate() > InitialBitrate);
    }

    #[test]
    fn EstimateNotLimitedByNetworkEstimateIfDisabled() {
        // WebRTC-Bwe-EstimateBoundedIncrease/Disabled/
        let field_trials = FieldTrials {
            estimate_bounded_increase: EstimateBoundedIncrease {
                disable_estimate_bounded_increase: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut aimd_rate_control = AimdRateControl::new(&field_trials, true);
        aimd_rate_control.disable_estimate_bounded_increase = true;

        let mut now: Timestamp = InitialTime;
        const InitialBitrate: DataRate = DataRate::BitsPerSec(123_000);
        aimd_rate_control.SetEstimate(InitialBitrate, now);
        aimd_rate_control.SetInApplicationLimitedRegion(false);
        let mut network_estimate = NetworkStateEstimate::default();
        network_estimate.link_capacity_upper = DataRate::KilobitsPerSec(150);
        aimd_rate_control.SetNetworkStateEstimate(Some(network_estimate));

        for i in 0..100 {
            aimd_rate_control.Update(RateControlInput::new(BandwidthUsage::Normal, None), now);
            now += TimeDelta::Millis(100);
        }
        assert!(aimd_rate_control.LatestEstimate() > network_estimate.link_capacity_upper);
    }
}
