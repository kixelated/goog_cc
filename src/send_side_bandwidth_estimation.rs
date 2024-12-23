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

use crate::{
    api::{
        transport::{BandwidthUsage, SentPacket, TransportPacketsFeedback},
        units::{DataRate, TimeDelta, Timestamp},
    },
    LossBasedBandwidthEstimation, LossBasedBweV2, LossBasedState,
};

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
        todo!();
    }
    pub fn OnStartingRate(&mut self, start_rate: DataRate) {
        todo!();
    }
    pub fn OnRateUpdate(
        &mut self,
        acknowledged: Option<DataRate>,
        target: DataRate,
        at_time: Timestamp,
    ) {
        todo!();
    }
    pub fn OnRttBackoff(&mut self, backoff_rate: DataRate, at_time: Timestamp) {
        todo!();
    }
    pub fn estimate(&self) -> DataRate {
        todo!();
    }
}

// WebRTC-Bwe-MaxRttLimit
#[derive(Clone, Debug)]
pub struct RttBasedBackoffConfig {
    disabled: bool, // Disabled
    configured_limit: TimeDelta, // limit
    drop_fraction: f64, // fraction
    drop_interval: TimeDelta, // interval,
    bandwidth_floor: DataRate, // floor
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
            rtt_limit: if !config.disabled { config.configured_limit } else { TimeDelta::Zero() },
            last_propagation_rtt_update: Timestamp::PlusInfinity(),
            last_propagation_rtt: TimeDelta::Zero(),
            last_packet_sent: Timestamp::MinusInfinity(),
        }
    }

    pub fn UpdatePropagationRtt(&mut self, at_time: Timestamp, propagation_rtt: TimeDelta) {
        todo!();
    }
    pub fn IsRttAboveLimit(&self) -> bool {
        todo!();
    }

    fn CorrectedRtt(&self) -> TimeDelta {
        todo!()
    }
}

enum UmaState {
    NoUpdate,
    FirstDone,
    Done,
}

pub struct SendSideBandwidthEstimation {
    rtt_backoff: RttBasedBackoff,
    link_capacity: LinkCapacityTracker,

    min_bitrate_history: VecDeque<(Timestamp, DataRate)>,

    // incoming filters
    lost_packets_since_last_loss_update: isize,
    expected_packets_since_last_loss_update: isize,

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
    initially_lost_packets: isize,
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
        todo!()
    }
}

impl SendSideBandwidthEstimation {
    pub fn OnRouteChange(&mut self) {
        todo!();
    }

    pub fn target_rate(&self) -> DataRate {
        todo!();
    }
    pub fn loss_based_state(&self) -> LossBasedState {
        todo!();
    }
    // Return whether the current rtt is higher than the rtt limited configured in
    // RttBasedBackoff.
    pub fn IsRttAboveLimit(&self) -> bool {
        todo!();
    }

    pub fn fraction_loss(&self) -> u8 {
        self.last_fraction_loss
    }

    pub fn round_trip_time(&self) -> TimeDelta {
        self.last_round_trip_time
    }

    pub fn GetEstimatedLinkCapacity(&self) -> DataRate {
        todo!();
    }
    // Call periodically to update estimate.
    pub fn UpdateEstimate(&mut self, at_time: Timestamp) {
        todo!();
    }
    pub fn OnSentPacket(&mut self, sent_packet: SentPacket) {
        todo!();
    }
    pub fn UpdatePropagationRtt(&mut self, at_time: Timestamp, propagation_rtt: TimeDelta) {
        todo!();
    }
    // Call when we receive a RTCP message with TMMBR or REMB.
    pub fn UpdateReceiverEstimate(&mut self, at_time: Timestamp, bandwidth: DataRate) {
        todo!();
    }
    // Call when a new delay-based estimate is available.
    pub fn UpdateDelayBasedEstimate(&mut self, at_time: Timestamp, bitrate: DataRate) {
        todo!();
    }
    // Call when we receive a RTCP message with a ReceiveBlock.
    pub fn UpdatePacketsLost(
        &mut self,
        packets_lost: i64,
        number_of_packets: i64,
        at_time: Timestamp,
    ) {
        todo!();
    }
    // Call when we receive a RTCP message with a ReceiveBlock.
    pub fn UpdateRtt(&mut self, rtt: TimeDelta, at_time: Timestamp) {
        todo!();
    }
    pub fn SetBitrates(
        &mut self,
        send_bitrate: Option<DataRate>,
        min_bitrate: DataRate,
        max_bitrate: DataRate,
        at_time: Timestamp,
    ) {
        todo!();
    }
    pub fn SetSendBitrate(&mut self, bitrate: DataRate, at_time: Timestamp) {
        todo!();
    }
    pub fn SetMinMaxBitrate(&mut self, min_bitrate: DataRate, max_bitrate: DataRate) {
        todo!();
    }

    pub fn GetMinBitrate(&self) -> isize {
        todo!();
    }

    pub fn SetAcknowledgedRate(&mut self, acknowledged_rate: Option<DataRate>, at_time: Timestamp) {
        todo!();
    }
    pub fn UpdateLossBasedEstimator(
        &mut self,
        report: &TransportPacketsFeedback,
        delay_detector_state: BandwidthUsage,
        probe_bitrate: Option<DataRate>,
        in_alr: bool,
    ) {
        todo!();
    }
    pub fn PaceAtLossBasedEstimate(&self) -> bool {
        todo!();
    }

    fn IsInStartPhase(&self, at_time: Timestamp) -> bool {
        todo!();
    }

    fn UpdateUmaStatsPacketsLost(at_time: Timestamp, packets_lost: isize) {
        todo!();
    }

    // Updates history of min bitrates.
    // After this method returns self.min_bitrate_history.front().second contains the
    // min bitrate used during last BweIncreaseIntervalMs.
    fn UpdateMinHistory(at_time: Timestamp) {
        todo!();
    }

    // Gets the upper limit for the target bitrate. This is the minimum of the
    // delay based limit, the receiver limit and the loss based controller limit.
    fn GetUpperLimit(&self) -> DataRate {
        todo!();
    }
    // Prints a warning if `bitrate` if sufficiently long time has past since last
    // warning.
    fn MaybeLogLowBitrateWarning(&self, bitrate: DataRate, at_time: Timestamp) {
        todo!();
    }
    // Stores an update to the event log if the loss rate has changed, the target
    // has changed, or sufficient time has passed since last stored event.
    fn MaybeLogLossBasedEvent(&self, at_time: Timestamp) {
        todo!();
    }

    // Cap `bitrate` to [self.min_bitrate_configured, max_bitrate_configured_] and
    // set `current_bitrate_` to the capped value and updates the event log.
    fn UpdateTargetBitrate(&mut self, bitrate: DataRate, at_time: Timestamp) {
        todo!();
    }
    // Applies lower and upper bounds to the current target rate.
    // TODO(srte): This seems to be called even when limits haven't changed, that
    // should be cleaned up.
    fn ApplyTargetLimits(&mut self, at_time: Timestamp) {
        todo!();
    }

    fn LossBasedBandwidthEstimatorV1Enabled(&self) -> bool {
        todo!();
    }
    fn LossBasedBandwidthEstimatorV2Enabled(&self) -> bool {
        todo!();
    }

    fn LossBasedBandwidthEstimatorV1ReadyForUse(&self) -> bool {
        todo!();
    }
    fn LossBasedBandwidthEstimatorV2ReadyForUse(&self) -> bool {
        todo!();
    }
}
