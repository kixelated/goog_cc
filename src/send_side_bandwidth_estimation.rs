/*
 *  Copyright (c) 2012 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 *
 *  FEC and NACK added bitrate is handled outside class
 */

use std::collections::VecDeque;

use crate::remote_bitrate_estimator::CongestionControllerMinBitrate;
use crate::{
    api::{
        transport::{BandwidthUsage, SentPacket, TransportPacketsFeedback},
        units::{DataRate, TimeDelta, Timestamp},
    },
    FieldTrials, LossBasedBandwidthEstimation, LossBasedBweV2, LossBasedState,
};

#[derive(Clone, Debug)]
pub struct BweLossExperiment {
    pub enabled: bool,
    pub low_loss_threshold: f32,
    pub high_loss_threshold: f32,
    pub bitrate_threshold_kbps: f64,
}

impl Default for BweLossExperiment {
    fn default() -> Self {
        Self {
            enabled: false,
            low_loss_threshold: 0.02,
            high_loss_threshold: 0.1,
            bitrate_threshold_kbps: 0.0,
        }
    }
}

pub struct LinkCapacityTracker {
    capacity_estimate_bps: f64,
    last_link_capacity_update: Timestamp, // = Timestamp::MinusInfinity();
    last_delay_based_estimate: DataRate,  // = DataRate::PlusInfinity();
}

impl Default for LinkCapacityTracker {
    fn default() -> Self {
        Self {
            capacity_estimate_bps: 0.0,
            last_link_capacity_update: Timestamp::MinusInfinity(),
            last_delay_based_estimate: DataRate::PlusInfinity(),
        }
    }
}

impl LinkCapacityTracker {
    // Call when a new delay-based estimate is available.
    pub fn UpdateDelayBasedEstimate(&mut self, at_time: Timestamp, delay_based_bitrate: DataRate) {
        if delay_based_bitrate < self.last_delay_based_estimate {
            self.capacity_estimate_bps = self
                .capacity_estimate_bps
                .min(delay_based_bitrate.bps_float());
            self.last_link_capacity_update = at_time;
        }
        self.last_delay_based_estimate = delay_based_bitrate;
    }
    pub fn OnStartingRate(&mut self, start_rate: DataRate) {
        if self.last_link_capacity_update.IsInfinite() {
            self.capacity_estimate_bps = start_rate.bps_float();
        }
    }
    pub fn OnRateUpdate(
        &mut self,
        acknowledged: Option<DataRate>,
        target: DataRate,
        at_time: Timestamp,
    ) {
        let acknowledged = match acknowledged {
            Some(ack) => ack,
            None => return,
        };
        let acknowledged_target: DataRate = std::cmp::min(acknowledged, target);
        if acknowledged_target.bps_float() > self.capacity_estimate_bps {
            let delta: TimeDelta = at_time - self.last_link_capacity_update;
            let alpha: f64 = if delta.IsFinite() {
                (-(delta / TimeDelta::Seconds(10))).exp()
            } else {
                0.0
            };
            self.capacity_estimate_bps = alpha * self.capacity_estimate_bps
                + (1.0 - alpha) * acknowledged_target.bps_float();
        }
        self.last_link_capacity_update = at_time;
    }
    pub fn OnRttBackoff(&mut self, backoff_rate: DataRate, at_time: Timestamp) {
        self.capacity_estimate_bps = self.capacity_estimate_bps.min(backoff_rate.bps_float());
        self.last_link_capacity_update = at_time;
    }
    pub fn estimate(&self) -> DataRate {
        DataRate::BitsPerSecFloat(self.capacity_estimate_bps)
    }
}

// WebRTC-Bwe-MaxRttLimit
#[derive(Clone, Debug)]
pub struct RttBasedBackoffConfig {
    disabled: bool,              // Disabled
    configured_limit: TimeDelta, // limit
    drop_fraction: f64,          // fraction
    drop_interval: TimeDelta,    // interval,
    bandwidth_floor: DataRate,   // floor
}

impl Default for RttBasedBackoffConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            configured_limit: TimeDelta::Seconds(3),
            drop_fraction: 0.8,
            drop_interval: TimeDelta::Seconds(1),
            bandwidth_floor: DataRate::KilobitsPerSec(5),
        }
    }
}

pub struct RttBasedBackoff {
    pub disabled: bool,
    pub configured_limit: TimeDelta,
    pub drop_fraction: f64,
    pub drop_interval: TimeDelta,
    pub bandwidth_floor: DataRate,
    pub rtt_limit: TimeDelta,
    pub last_propagation_rtt_update: Timestamp,
    pub last_propagation_rtt: TimeDelta,
    pub last_packet_sent: Timestamp,
}

impl RttBasedBackoff {
    pub fn new(config: &RttBasedBackoffConfig) -> Self {
        Self {
            disabled: config.disabled,
            configured_limit: config.configured_limit,
            drop_fraction: config.drop_fraction,
            drop_interval: config.drop_interval,
            bandwidth_floor: config.bandwidth_floor,
            rtt_limit: if !config.disabled {
                config.configured_limit
            } else {
                TimeDelta::Zero()
            },
            last_propagation_rtt_update: Timestamp::PlusInfinity(),
            last_propagation_rtt: TimeDelta::Zero(),
            last_packet_sent: Timestamp::MinusInfinity(),
        }
    }

    pub fn UpdatePropagationRtt(&mut self, at_time: Timestamp, propagation_rtt: TimeDelta) {
        self.last_propagation_rtt_update = at_time;
        self.last_propagation_rtt = propagation_rtt;
    }
    pub fn IsRttAboveLimit(&self) -> bool {
        self.CorrectedRtt() > self.rtt_limit
    }

    fn CorrectedRtt(&self) -> TimeDelta {
        // Avoid timeout when no packets are being sent.
        let timeout_correction: TimeDelta = std::cmp::max(
            self.last_packet_sent - self.last_propagation_rtt_update,
            TimeDelta::Zero(),
        );
        timeout_correction + self.last_propagation_rtt
    }
}

enum UmaState {
    NoUpdate,
    FirstDone,
    Done,
}

pub struct SendSideBandwidthEstimation {
    field_trials: FieldTrials,
    rtt_backoff: RttBasedBackoff,
    link_capacity: LinkCapacityTracker,

    min_bitrate_history: VecDeque<(Timestamp, DataRate)>,

    // incoming filters
    lost_packets_since_last_loss_update: i64,
    expected_packets_since_last_loss_update: i64,

    acknowledged_rate: Option<DataRate>,
    current_target: DataRate,
    last_logged_target: DataRate,
    min_bitrate_configured: DataRate,
    max_bitrate_configured: DataRate,
    last_low_bitrate_log: Timestamp,

    has_decreased_since_last_fraction_loss: bool,
    last_loss_feedback: Timestamp,
    last_loss_packet_report: Timestamp,
    last_fraction_loss: u8,
    last_logged_fraction_loss: u8,
    last_round_trip_time: TimeDelta,

    // The max bitrate as set by the receiver in the call. This is typically
    // signalled using the REMB RTCP message and is used when we don't have any
    // send side delay based estimate.
    receiver_limit: DataRate,
    delay_based_limit: DataRate,
    time_last_decrease: Timestamp,
    first_report_time: Timestamp,
    initially_lost_packets: i64,
    bitrate_at_2_seconds: DataRate,
    uma_update_state: UmaState,
    uma_rtt_state: UmaState,
    rampup_uma_stats_updated: Vec<bool>,
    last_rtc_event_log: Timestamp,
    low_loss_threshold: f32,
    high_loss_threshold: f32,
    bitrate_threshold: DataRate,
    loss_based_bandwidth_estimator_v1: LossBasedBandwidthEstimation,
    loss_based_bandwidth_estimator_v2: LossBasedBweV2,
    loss_based_state: LossBasedState,
    disable_receiver_limit_caps_only: bool,
}

impl Default for SendSideBandwidthEstimation {
    fn default() -> Self {
        Self::new(FieldTrials::default())
    }
}

impl SendSideBandwidthEstimation {
    const BweIncreaseInterval: TimeDelta = TimeDelta::Millis(1000);
    const BweDecreaseInterval: TimeDelta = TimeDelta::Millis(300);
    const StartPhase: TimeDelta = TimeDelta::Millis(2000);
    const BweConverganceTime: TimeDelta = TimeDelta::Millis(20000);
    const LimitNumPackets: i64 = 20;
    const DefaultMaxBitrate: DataRate = DataRate::BitsPerSec(1000000000);
    const LowBitrateLogPeriod: TimeDelta = TimeDelta::Millis(10000);
    const RtcEventLogPeriod: TimeDelta = TimeDelta::Millis(5000);
    // Expecting that RTCP feedback is sent uniformly within [0.5, 1.5]s intervals.
    const MaxRtcpFeedbackInterval: TimeDelta = TimeDelta::Millis(5000);

    pub fn new(field_trials: FieldTrials) -> Self {
        let mut low_loss_threshold = 0.0;
        let mut high_loss_threshold = 0.0;
        let mut bitrate_threshold_kbps = 0.0;

        if field_trials.loss_experiment.enabled {
            low_loss_threshold = field_trials.loss_experiment.low_loss_threshold;
            high_loss_threshold = field_trials.loss_experiment.high_loss_threshold;
            bitrate_threshold_kbps = field_trials.loss_experiment.bitrate_threshold_kbps;
            assert!(low_loss_threshold > 0.0);
            assert!(low_loss_threshold <= 1.0);
            assert!(high_loss_threshold > 0.0);
            assert!(high_loss_threshold <= 1.0);
            assert!(low_loss_threshold <= high_loss_threshold);
            assert!(bitrate_threshold_kbps >= 0.0);
            tracing::info!(
                "Enabled BweLossExperiment with parameters {:?}, {:?}, {:?}",
                low_loss_threshold,
                high_loss_threshold,
                bitrate_threshold_kbps
            );
        }
        let bitrate_threshold = DataRate::KilobitsPerSecFloat(bitrate_threshold_kbps);

        let mut loss_based_bandwidth_estimator_v2 =
            LossBasedBweV2::new(field_trials.loss_based_bwe_v2.clone());
        if field_trials.loss_based_bwe_v2.enabled {
            loss_based_bandwidth_estimator_v2
                .SetMinMaxBitrate(CongestionControllerMinBitrate, Self::DefaultMaxBitrate);
        }

        Self {
            rtt_backoff: RttBasedBackoff::new(&field_trials.max_rtt_limit),
            link_capacity: LinkCapacityTracker::default(),
            min_bitrate_history: VecDeque::new(),
            lost_packets_since_last_loss_update: 0,
            expected_packets_since_last_loss_update: 0,
            current_target: DataRate::Zero(),
            last_logged_target: DataRate::Zero(),
            min_bitrate_configured: CongestionControllerMinBitrate,
            max_bitrate_configured: Self::DefaultMaxBitrate,
            last_low_bitrate_log: Timestamp::MinusInfinity(),
            has_decreased_since_last_fraction_loss: false,
            last_loss_feedback: Timestamp::MinusInfinity(),
            last_loss_packet_report: Timestamp::MinusInfinity(),
            last_fraction_loss: 0,
            last_logged_fraction_loss: 0,
            last_round_trip_time: TimeDelta::Zero(),
            receiver_limit: DataRate::PlusInfinity(),
            delay_based_limit: DataRate::PlusInfinity(),
            time_last_decrease: Timestamp::MinusInfinity(),
            first_report_time: Timestamp::MinusInfinity(),
            initially_lost_packets: 0,
            bitrate_at_2_seconds: DataRate::Zero(),
            uma_update_state: UmaState::NoUpdate,
            uma_rtt_state: UmaState::NoUpdate,
            rampup_uma_stats_updated: vec![false; 0],
            last_rtc_event_log: Timestamp::MinusInfinity(),
            low_loss_threshold,
            high_loss_threshold,
            bitrate_threshold,
            loss_based_bandwidth_estimator_v1: LossBasedBandwidthEstimation::new(
                field_trials.loss_based_control.clone(),
            ),
            loss_based_bandwidth_estimator_v2,
            loss_based_state: LossBasedState::DelayBasedEstimate,
            disable_receiver_limit_caps_only: field_trials.receiver_limit_caps_only,
            acknowledged_rate: None,
            field_trials,
        }
    }

    pub fn OnRouteChange(&mut self) {
        self.lost_packets_since_last_loss_update = 0;
        self.expected_packets_since_last_loss_update = 0;
        self.current_target = DataRate::Zero();
        self.min_bitrate_configured = CongestionControllerMinBitrate;
        self.max_bitrate_configured = Self::DefaultMaxBitrate;
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
        self.uma_update_state = UmaState::NoUpdate;
        self.uma_rtt_state = UmaState::NoUpdate;
        self.last_rtc_event_log = Timestamp::MinusInfinity();
        if self.LossBasedBandwidthEstimatorV2Enabled()
            && self.loss_based_bandwidth_estimator_v2.UseInStartPhase()
        {
            self.loss_based_bandwidth_estimator_v2 =
                LossBasedBweV2::new(self.field_trials.loss_based_bwe_v2.clone());
        }
    }

    pub fn target_rate(&self) -> DataRate {
        let mut target: DataRate = self.current_target;
        if !self.disable_receiver_limit_caps_only {
            target = std::cmp::min(target, self.receiver_limit);
        }
        std::cmp::max(self.min_bitrate_configured, target)
    }
    pub fn loss_based_state(&self) -> LossBasedState {
        self.loss_based_state
    }
    // Return whether the current rtt is higher than the rtt limited configured in
    // RttBasedBackoff.
    pub fn IsRttAboveLimit(&self) -> bool {
        self.rtt_backoff.IsRttAboveLimit()
    }

    pub fn fraction_loss(&self) -> u8 {
        self.last_fraction_loss
    }

    pub fn round_trip_time(&self) -> TimeDelta {
        self.last_round_trip_time
    }

    pub fn GetEstimatedLinkCapacity(&self) -> DataRate {
        self.link_capacity.estimate()
    }
    // Call periodically to update estimate.
    pub fn UpdateEstimate(&mut self, at_time: Timestamp) {
        if self.rtt_backoff.IsRttAboveLimit() {
            if at_time - self.time_last_decrease >= self.rtt_backoff.drop_interval
                && self.current_target > self.rtt_backoff.bandwidth_floor
            {
                self.time_last_decrease = at_time;
                let new_bitrate: DataRate = std::cmp::max(
                    self.current_target * self.rtt_backoff.drop_fraction,
                    self.rtt_backoff.bandwidth_floor,
                );
                self.link_capacity.OnRttBackoff(new_bitrate, at_time);
                self.UpdateTargetBitrate(new_bitrate, at_time);
                return;
            }
            // TODO(srte): This is likely redundant in most cases.
            self.ApplyTargetLimits(at_time);
            return;
        }

        // We trust the REMB and/or delay-based estimate during the first 2 seconds if
        // we haven't had any packet loss reported, to allow startup bitrate probing.
        if self.last_fraction_loss == 0
            && self.IsInStartPhase(at_time)
            && !self
                .loss_based_bandwidth_estimator_v2
                .ReadyToUseInStartPhase()
        {
            let mut new_bitrate: DataRate = self.current_target;
            // TODO(srte): We should not allow the new_bitrate to be larger than the
            // receiver limit here.
            if self.receiver_limit.IsFinite() {
                new_bitrate = std::cmp::max(self.receiver_limit, new_bitrate);
            }
            if self.delay_based_limit.IsFinite() {
                new_bitrate = std::cmp::max(self.delay_based_limit, new_bitrate);
            }
            if self.LossBasedBandwidthEstimatorV1Enabled() {
                self.loss_based_bandwidth_estimator_v1
                    .Initialize(new_bitrate);
            }

            if new_bitrate != self.current_target {
                self.min_bitrate_history.clear();
                if self.LossBasedBandwidthEstimatorV1Enabled() {
                    self.min_bitrate_history.push_back((at_time, new_bitrate));
                } else {
                    self.min_bitrate_history
                        .push_back((at_time, self.current_target));
                }
                self.UpdateTargetBitrate(new_bitrate, at_time);
                return;
            }
        }
        self.UpdateMinHistory(at_time);
        if self.last_loss_packet_report.IsInfinite() {
            // No feedback received.
            // TODO(srte): This is likely redundant in most cases.
            self.ApplyTargetLimits(at_time);
            return;
        }

        if self.LossBasedBandwidthEstimatorV1ReadyForUse() {
            let new_bitrate: DataRate = self.loss_based_bandwidth_estimator_v1.Update(
                at_time,
                self.min_bitrate_history.front().unwrap().1,
                self.delay_based_limit,
                self.last_round_trip_time,
            );
            self.UpdateTargetBitrate(new_bitrate, at_time);
            return;
        }

        if self.LossBasedBandwidthEstimatorV2ReadyForUse() {
            let result = self.loss_based_bandwidth_estimator_v2.GetLossBasedResult();
            self.loss_based_state = result.state;
            self.UpdateTargetBitrate(result.bandwidth_estimate, at_time);
            return;
        }

        let time_since_loss_packet_report: TimeDelta = at_time - self.last_loss_packet_report;
        if time_since_loss_packet_report < 1.2 * Self::MaxRtcpFeedbackInterval {
            // We only care about loss above a given bitrate threshold.
            let loss: f32 = self.last_fraction_loss as f32 / 256.0;
            // We only make decisions based on loss when the bitrate is above a
            // threshold. This is a crude way of handling loss which is uncorrelated
            // to congestion.
            if self.current_target < self.bitrate_threshold || loss <= self.low_loss_threshold {
                // Loss < 2%: Increase rate by 8% of the min bitrate in the last
                // BweIncreaseInterval.
                // Note that by remembering the bitrate over the last second one can
                // rampup up one second faster than if only allowed to start ramping
                // at 8% per second rate now. E.g.:
                //   If sending a constant 100kbps it can rampup immediately to 108kbps
                //   whenever a receiver report is received with lower packet loss.
                //   If instead one would do: self.current_bitrate *= 1.08^(delta time),
                //   it would take over one second since the lower packet loss to achieve
                //   108kbps.
                let mut new_bitrate: DataRate = DataRate::BitsPerSecFloat(
                    self.min_bitrate_history.front().unwrap().1.bps_float() * 1.08 + 0.5,
                );

                // Add 1 kbps extra, just to make sure that we do not get stuck
                // (gives a little extra increase at low rates, negligible at higher
                // rates).
                new_bitrate += DataRate::BitsPerSec(1000);
                self.UpdateTargetBitrate(new_bitrate, at_time);
                return;
            } else if self.current_target > self.bitrate_threshold {
                if loss <= self.high_loss_threshold {
                    // Loss between 2% - 10%: Do nothing.
                } else {
                    // Loss > 10%: Limit the rate decreases to once a BweDecreaseInterval
                    // + rtt.
                    if !self.has_decreased_since_last_fraction_loss
                        && (at_time - self.time_last_decrease)
                            >= (Self::BweDecreaseInterval + self.last_round_trip_time)
                    {
                        self.time_last_decrease = at_time;

                        // Reduce rate:
                        //   newRate = rate * (1 - 0.5*lossRate);
                        //   where packetLoss = 256*lossRate;
                        let new_bitrate: DataRate = DataRate::BitsPerSecFloat(
                            (self.current_target.bps_float()
                                * (512.0 - self.last_fraction_loss as f64))
                                / 512.0,
                        );
                        self.has_decreased_since_last_fraction_loss = true;
                        self.UpdateTargetBitrate(new_bitrate, at_time);
                        return;
                    }
                }
            }
        }
        // TODO(srte): This is likely redundant in most cases.
        self.ApplyTargetLimits(at_time);
    }
    pub fn OnSentPacket(&mut self, sent_packet: SentPacket) {
        // Only feedback-triggering packets will be reported here.
        self.rtt_backoff.last_packet_sent = sent_packet.send_time;
    }
    pub fn UpdatePropagationRtt(&mut self, at_time: Timestamp, propagation_rtt: TimeDelta) {
        self.rtt_backoff
            .UpdatePropagationRtt(at_time, propagation_rtt);
    }
    // Call when we receive a RTCP message with TMMBR or REMB.
    pub fn UpdateReceiverEstimate(&mut self, at_time: Timestamp, bandwidth: DataRate) {
        // TODO(srte): Ensure caller passes PlusInfinity, not zero, to represent no
        // limitation.
        self.receiver_limit = if bandwidth.IsZero() {
            DataRate::PlusInfinity()
        } else {
            bandwidth
        };
        self.ApplyTargetLimits(at_time);
    }
    // Call when a new delay-based estimate is available.
    pub fn UpdateDelayBasedEstimate(&mut self, at_time: Timestamp, bitrate: DataRate) {
        self.link_capacity
            .UpdateDelayBasedEstimate(at_time, bitrate);
        // TODO(srte): Ensure caller passes PlusInfinity, not zero, to represent no
        // limitation.
        self.delay_based_limit = if bitrate.IsZero() {
            DataRate::PlusInfinity()
        } else {
            bitrate
        };
        self.ApplyTargetLimits(at_time);
    }
    // Call when we receive a RTCP message with a ReceiveBlock.
    pub fn UpdatePacketsLost(
        &mut self,
        packets_lost: i64,
        number_of_packets: i64,
        at_time: Timestamp,
    ) {
        self.last_loss_feedback = at_time;
        if self.first_report_time.IsInfinite() {
            self.first_report_time = at_time;
        }

        // Check sequence number diff and weight loss report
        if number_of_packets > 0 {
            let expected: i64 = self.expected_packets_since_last_loss_update + number_of_packets;

            // Don't generate a loss rate until it can be based on enough packets.
            if expected < Self::LimitNumPackets {
                // Accumulate reports.
                self.expected_packets_since_last_loss_update = expected;
                self.lost_packets_since_last_loss_update += packets_lost;
                return;
            }

            self.has_decreased_since_last_fraction_loss = false;
            let lost_q8: i64 =
                std::cmp::max(self.lost_packets_since_last_loss_update + packets_lost, 0) << 8;
            self.last_fraction_loss = (lost_q8 / expected).min(255) as u8;

            // Reset accumulators.
            self.lost_packets_since_last_loss_update = 0;
            self.expected_packets_since_last_loss_update = 0;
            self.last_loss_packet_report = at_time;
            self.UpdateEstimate(at_time);
        }
    }
    // Call when we receive a RTCP message with a ReceiveBlock.
    pub fn UpdateRtt(&mut self, rtt: TimeDelta, at_time: Timestamp) {
        // Update RTT if we were able to compute an RTT based on this RTCP.
        // FlexFEC doesn't send RTCP SR, which means we won't be able to compute RTT.
        if rtt > TimeDelta::Zero() {
            self.last_round_trip_time = rtt;
        }
    }
    pub fn SetBitrates(
        &mut self,
        send_bitrate: Option<DataRate>,
        min_bitrate: DataRate,
        max_bitrate: DataRate,
        at_time: Timestamp,
    ) {
        self.SetMinMaxBitrate(min_bitrate, max_bitrate);
        if let Some(send_bitrate) = send_bitrate {
            self.link_capacity.OnStartingRate(send_bitrate);
            self.SetSendBitrate(send_bitrate, at_time);
        }
    }
    pub fn SetSendBitrate(&mut self, bitrate: DataRate, at_time: Timestamp) {
        assert!(bitrate > DataRate::Zero());
        // Reset to avoid being capped by the estimate.
        self.delay_based_limit = DataRate::PlusInfinity();
        self.UpdateTargetBitrate(bitrate, at_time);
        // Clear last sent bitrate history so the new value can be used directly
        // and not capped.
        self.min_bitrate_history.clear();
    }
    pub fn SetMinMaxBitrate(&mut self, min_bitrate: DataRate, max_bitrate: DataRate) {
        self.min_bitrate_configured = std::cmp::max(min_bitrate, CongestionControllerMinBitrate);
        if max_bitrate > DataRate::Zero() && max_bitrate.IsFinite() {
            self.max_bitrate_configured = std::cmp::max(self.min_bitrate_configured, max_bitrate);
        } else {
            self.max_bitrate_configured = Self::DefaultMaxBitrate;
        }
        self.loss_based_bandwidth_estimator_v2
            .SetMinMaxBitrate(self.min_bitrate_configured, self.max_bitrate_configured);
    }

    pub fn GetMinBitrate(&self) -> i64 {
        self.min_bitrate_configured.bps()
    }

    pub fn SetAcknowledgedRate(&mut self, acknowledged_rate: Option<DataRate>, at_time: Timestamp) {
        self.acknowledged_rate = acknowledged_rate;
        let acknowledged_rate = match acknowledged_rate {
            Some(rate) => rate,
            None => return,
        };
        if self.LossBasedBandwidthEstimatorV1Enabled() {
            self.loss_based_bandwidth_estimator_v1
                .UpdateAcknowledgedBitrate(acknowledged_rate, at_time);
        }
        if self.LossBasedBandwidthEstimatorV2Enabled() {
            self.loss_based_bandwidth_estimator_v2
                .SetAcknowledgedBitrate(acknowledged_rate);
        }
    }
    pub fn UpdateLossBasedEstimator(
        &mut self,
        report: &TransportPacketsFeedback,
        delay_detector_state: BandwidthUsage,
        probe_bitrate: Option<DataRate>,
        in_alr: bool,
    ) {
        if self.LossBasedBandwidthEstimatorV1Enabled() {
            self.loss_based_bandwidth_estimator_v1
                .UpdateLossStatistics(&report.packet_feedbacks, report.feedback_time);
        }
        if self.LossBasedBandwidthEstimatorV2Enabled() {
            self.loss_based_bandwidth_estimator_v2
                .UpdateBandwidthEstimate(&report.packet_feedbacks, self.delay_based_limit, in_alr);
            self.UpdateEstimate(report.feedback_time);
        }
    }
    pub fn PaceAtLossBasedEstimate(&self) -> bool {
        self.LossBasedBandwidthEstimatorV2ReadyForUse()
            && self
                .loss_based_bandwidth_estimator_v2
                .PaceAtLossBasedEstimate()
    }

    fn IsInStartPhase(&self, at_time: Timestamp) -> bool {
        self.first_report_time.IsInfinite() || at_time - self.first_report_time < Self::StartPhase
    }

    // Updates history of min bitrates.
    // After this method returns self.min_bitrate_history.front().second contains the
    // min bitrate used during last BweIncreaseIntervalMs.
    fn UpdateMinHistory(&mut self, at_time: Timestamp) {
        // Remove old data points from history.
        // Since history precision is in ms, add one so it is able to increase
        // bitrate if it is off by as little as 0.5ms.
        while !self.min_bitrate_history.is_empty()
            && at_time - self.min_bitrate_history.front().unwrap().0 + TimeDelta::Millis(1)
                > Self::BweIncreaseInterval
        {
            self.min_bitrate_history.pop_front();
        }

        // Typical minimum sliding-window algorithm: Pop values higher than current
        // bitrate before pushing it.
        while !self.min_bitrate_history.is_empty()
            && self.current_target <= self.min_bitrate_history.back().unwrap().1
        {
            self.min_bitrate_history.pop_back();
        }

        self.min_bitrate_history
            .push_back((at_time, self.current_target));
    }

    // Gets the upper limit for the target bitrate. This is the minimum of the
    // delay based limit, the receiver limit and the loss based controller limit.
    fn GetUpperLimit(&self) -> DataRate {
        let mut upper_limit: DataRate = self.delay_based_limit;
        if self.disable_receiver_limit_caps_only {
            upper_limit = std::cmp::min(upper_limit, self.receiver_limit);
        }
        std::cmp::min(upper_limit, self.max_bitrate_configured)
    }
    // Prints a warning if `bitrate` if sufficiently long time has past since last
    // warning.
    fn MaybeLogLowBitrateWarning(&mut self, bitrate: DataRate, at_time: Timestamp) {
        if at_time - self.last_low_bitrate_log > Self::LowBitrateLogPeriod {
            tracing::warn!(
                "Estimated available bandwidth {:?} is below configured min bitrate {:?}.",
                bitrate,
                self.min_bitrate_configured
            );
            self.last_low_bitrate_log = at_time;
        }
    }

    // Cap `bitrate` to [self.min_bitrate_configured, max_bitrate_configured_] and
    // set `current_bitrate_` to the capped value and updates the event log.
    fn UpdateTargetBitrate(&mut self, mut new_bitrate: DataRate, at_time: Timestamp) {
        new_bitrate = std::cmp::min(new_bitrate, self.GetUpperLimit());
        if new_bitrate < self.min_bitrate_configured {
            self.MaybeLogLowBitrateWarning(new_bitrate, at_time);
            new_bitrate = self.min_bitrate_configured;
        }
        self.current_target = new_bitrate;
        self.link_capacity
            .OnRateUpdate(self.acknowledged_rate, self.current_target, at_time);
    }
    // Applies lower and upper bounds to the current target rate.
    // TODO(srte): This seems to be called even when limits haven't changed, that
    // should be cleaned up.
    fn ApplyTargetLimits(&mut self, at_time: Timestamp) {
        self.UpdateTargetBitrate(self.current_target, at_time);
    }

    fn LossBasedBandwidthEstimatorV1Enabled(&self) -> bool {
        self.field_trials.loss_based_control.enabled && !self.field_trials.loss_based_bwe_v2.enabled
    }
    fn LossBasedBandwidthEstimatorV2Enabled(&self) -> bool {
        self.field_trials.loss_based_bwe_v2.enabled
    }

    fn LossBasedBandwidthEstimatorV1ReadyForUse(&self) -> bool {
        self.LossBasedBandwidthEstimatorV1Enabled()
            && self.loss_based_bandwidth_estimator_v1.InUse()
    }
    fn LossBasedBandwidthEstimatorV2ReadyForUse(&self) -> bool {
        self.loss_based_bandwidth_estimator_v2.IsReady()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn TestProbing(use_delay_based: bool) {
        let mut bwe = SendSideBandwidthEstimation::new(Default::default());
        let mut now_ms: i64 = 0;
        bwe.SetMinMaxBitrate(DataRate::BitsPerSec(100000), DataRate::BitsPerSec(1500000));
        bwe.SetSendBitrate(DataRate::BitsPerSec(200000), Timestamp::Millis(now_ms));

        const RembBps: i64 = 1000000;
        const SecondRembBps: i64 = RembBps + 500000;

        bwe.UpdatePacketsLost(
            /*packets_lost=*/ 0,
            /*number_of_packets=*/ 1,
            Timestamp::Millis(now_ms),
        );
        bwe.UpdateRtt(TimeDelta::Millis(50), Timestamp::Millis(now_ms));

        // Initial REMB applies immediately.
        if use_delay_based {
            bwe.UpdateDelayBasedEstimate(Timestamp::Millis(now_ms), DataRate::BitsPerSec(RembBps));
        } else {
            bwe.UpdateReceiverEstimate(Timestamp::Millis(now_ms), DataRate::BitsPerSec(RembBps));
        }
        bwe.UpdateEstimate(Timestamp::Millis(now_ms));
        assert_eq!(RembBps, bwe.target_rate().bps());

        // Second REMB doesn't apply immediately.
        now_ms += 2001;
        if use_delay_based {
            bwe.UpdateDelayBasedEstimate(
                Timestamp::Millis(now_ms),
                DataRate::BitsPerSec(SecondRembBps),
            );
        } else {
            bwe.UpdateReceiverEstimate(
                Timestamp::Millis(now_ms),
                DataRate::BitsPerSec(SecondRembBps),
            );
        }
        bwe.UpdateEstimate(Timestamp::Millis(now_ms));
        assert_eq!(RembBps, bwe.target_rate().bps());
    }

    #[test]
    fn InitialRembWithProbing() {
        TestProbing(false);
    }

    #[test]
    fn InitialDelayBasedBweWithProbing() {
        TestProbing(true);
    }

    #[test]
    fn DoesntReapplyBitrateDecreaseWithoutFollowingRemb() {
        let mut bwe = SendSideBandwidthEstimation::new(Default::default());
        const MinBitrateBps: i64 = 100000;
        const InitialBitrateBps: i64 = 1000000;
        let mut now_ms: i64 = 1000;
        bwe.SetMinMaxBitrate(
            DataRate::BitsPerSec(MinBitrateBps),
            DataRate::BitsPerSec(1500000),
        );
        bwe.SetSendBitrate(
            DataRate::BitsPerSec(InitialBitrateBps),
            Timestamp::Millis(now_ms),
        );

        const FractionLoss: u8 = 128;
        const RttMs: i64 = 50;
        now_ms += 10000;

        assert_eq!(InitialBitrateBps, bwe.target_rate().bps());
        assert_eq!(0, bwe.fraction_loss());
        assert_eq!(0, bwe.round_trip_time().ms());

        // Signal heavy loss to go down in bitrate.
        bwe.UpdatePacketsLost(
            /*packets_lost=*/ 50,
            /*number_of_packets=*/ 100,
            Timestamp::Millis(now_ms),
        );
        bwe.UpdateRtt(TimeDelta::Millis(RttMs), Timestamp::Millis(now_ms));

        // Trigger an update 2 seconds later to not be rate limited.
        now_ms += 1000;
        bwe.UpdateEstimate(Timestamp::Millis(now_ms));
        assert!(bwe.target_rate().bps() < InitialBitrateBps);
        // Verify that the obtained bitrate isn't hitting the min bitrate, or this
        // test doesn't make sense. If this ever happens, update the thresholds or
        // loss rates so that it doesn't hit min bitrate after one bitrate update.
        assert!(bwe.target_rate().bps() > MinBitrateBps);
        assert_eq!(FractionLoss, bwe.fraction_loss());
        assert_eq!(RttMs, bwe.round_trip_time().ms());

        // Triggering an update shouldn't apply further downgrade nor upgrade since
        // there's no intermediate receiver block received indicating whether this is
        // currently good or not.
        let last_bitrate_bps: i64 = bwe.target_rate().bps();
        // Trigger an update 2 seconds later to not be rate limited (but it still
        // shouldn't update).
        now_ms += 1000;
        bwe.UpdateEstimate(Timestamp::Millis(now_ms));

        assert_eq!(last_bitrate_bps, bwe.target_rate().bps());
        // The old loss rate should still be applied though.
        assert_eq!(FractionLoss, bwe.fraction_loss());
        assert_eq!(RttMs, bwe.round_trip_time().ms());
    }

    #[test]
    fn SettingSendBitrateOverridesDelayBasedEstimate() {
        let mut bwe = SendSideBandwidthEstimation::new(Default::default());
        const MinBitrateBps: i64 = 10000;
        const MaxBitrateBps: i64 = 10000000;
        const InitialBitrateBps: i64 = 300000;
        const DelayBasedBitrateBps: i64 = 350000;
        const ForcedHighBitrate: i64 = 2500000;

        let now_ms: i64 = 0;

        bwe.SetMinMaxBitrate(
            DataRate::BitsPerSec(MinBitrateBps),
            DataRate::BitsPerSec(MaxBitrateBps),
        );
        bwe.SetSendBitrate(
            DataRate::BitsPerSec(InitialBitrateBps),
            Timestamp::Millis(now_ms),
        );

        bwe.UpdateDelayBasedEstimate(
            Timestamp::Millis(now_ms),
            DataRate::BitsPerSec(DelayBasedBitrateBps),
        );
        bwe.UpdateEstimate(Timestamp::Millis(now_ms));
        assert!(bwe.target_rate().bps() >= InitialBitrateBps);
        assert!(bwe.target_rate().bps() <= DelayBasedBitrateBps);

        bwe.SetSendBitrate(
            DataRate::BitsPerSec(ForcedHighBitrate),
            Timestamp::Millis(now_ms),
        );
        assert_eq!(bwe.target_rate().bps(), ForcedHighBitrate);
    }

    #[test]
    fn DefaultEnabled() {
        let rtt_backoff = RttBasedBackoff::new(&RttBasedBackoffConfig::default());
        assert!(rtt_backoff.rtt_limit.IsFinite());
    }

    #[test]
    fn CanBeDisabled() {
        let config = RttBasedBackoffConfig {
            disabled: true,
            ..Default::default()
        };

        let rtt_backoff = RttBasedBackoff::new(&config);
        assert!(rtt_backoff.rtt_limit.IsPlusInfinity());
    }

    #[test]
    fn FractionLossIsNotOverflowed() {
        let mut bwe = SendSideBandwidthEstimation::new(Default::default());
        const MinBitrateBps: i64 = 100000;
        const InitialBitrateBps: i64 = 1000000;
        let mut now_ms: i64 = 1000;
        bwe.SetMinMaxBitrate(
            DataRate::BitsPerSec(MinBitrateBps),
            DataRate::BitsPerSec(1500000),
        );
        bwe.SetSendBitrate(
            DataRate::BitsPerSec(InitialBitrateBps),
            Timestamp::Millis(now_ms),
        );

        now_ms += 10000;

        assert_eq!(InitialBitrateBps, bwe.target_rate().bps());
        assert_eq!(0, bwe.fraction_loss());

        // Signal negative loss.
        bwe.UpdatePacketsLost(
            /*packets_lost=*/ -1,
            /*number_of_packets=*/ 100,
            Timestamp::Millis(now_ms),
        );
        assert_eq!(0, bwe.fraction_loss());
    }

    #[test]
    fn RttIsAboveLimitIfRttGreaterThanLimit() {
        let mut bwe = SendSideBandwidthEstimation::new(Default::default());
        const MinBitrateBps: i64 = 10000;
        const MaxBitrateBps: i64 = 10000000;
        const InitialBitrateBps: i64 = 300000;
        let now_ms: i64 = 0;
        bwe.SetMinMaxBitrate(
            DataRate::BitsPerSec(MinBitrateBps),
            DataRate::BitsPerSec(MaxBitrateBps),
        );
        bwe.SetSendBitrate(
            DataRate::BitsPerSec(InitialBitrateBps),
            Timestamp::Millis(now_ms),
        );
        bwe.UpdatePropagationRtt(
            /*at_time=*/ Timestamp::Millis(now_ms),
            /*propagation_rtt=*/ TimeDelta::Millis(5000),
        );
        assert!(bwe.IsRttAboveLimit());
    }

    #[test]
    fn RttIsBelowLimitIfRttLessThanLimit() {
        let mut bwe = SendSideBandwidthEstimation::new(Default::default());
        const MinBitrateBps: i64 = 10000;
        const MaxBitrateBps: i64 = 10000000;
        const InitialBitrateBps: i64 = 300000;
        let now_ms: i64 = 0;
        bwe.SetMinMaxBitrate(
            DataRate::BitsPerSec(MinBitrateBps),
            DataRate::BitsPerSec(MaxBitrateBps),
        );
        bwe.SetSendBitrate(
            DataRate::BitsPerSec(InitialBitrateBps),
            Timestamp::Millis(now_ms),
        );
        bwe.UpdatePropagationRtt(
            /*at_time=*/ Timestamp::Millis(now_ms),
            /*propagation_rtt=*/ TimeDelta::Millis(1000),
        );
        assert!(!bwe.IsRttAboveLimit());
    }
}
