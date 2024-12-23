/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::api::{
    transport::PacketResult,
    units::{DataRate, TimeDelta, Timestamp},
};

#[derive(Clone, Debug)]
pub struct LossBasedControlConfig {
    pub enabled: bool, // Enabled
    pub min_increase_factor: f64, // min_incr
    pub max_increase_factor: f64, // max_incr
    pub increase_low_rtt: TimeDelta, // incr_low_rtt
    pub increase_high_rtt: TimeDelta, // incr_high_rtt
    pub decrease_factor: f64, // decr
    pub loss_window: TimeDelta, // loss_win
    pub loss_max_window: TimeDelta, // loss_max_win
    pub acknowledged_rate_max_window: TimeDelta, // ackrate_max_win
    pub increase_offset: DataRate, // incr_offset
    pub loss_bandwidth_balance_increase: DataRate, // balance_incr
    pub loss_bandwidth_balance_decrease: DataRate, // balance_decr
    pub loss_bandwidth_balance_reset: DataRate, // balance_reset
    pub loss_bandwidth_balance_exponent: f64, // exponent
    pub allow_resets: bool, // resets
    pub decrease_interval: TimeDelta, // decr_intvl
    pub loss_report_timeout: TimeDelta, // timeout
}

impl Default for LossBasedControlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_increase_factor: 1.02,
            max_increase_factor: 1.08,
            increase_low_rtt: TimeDelta::Millis(200),
            increase_high_rtt: TimeDelta::Millis(800),
            decrease_factor: 0.99,
            loss_window: TimeDelta::Millis(800),
            loss_max_window: TimeDelta::Millis(800),
            acknowledged_rate_max_window: TimeDelta::Millis(800),
            increase_offset: DataRate::BitsPerSec(1000),
            loss_bandwidth_balance_increase: DataRate::KilobitsPerSecFloat(0.5),
            loss_bandwidth_balance_decrease: DataRate::KilobitsPerSec(4),
            loss_bandwidth_balance_reset: DataRate::KilobitsPerSecFloat(0.1),
            loss_bandwidth_balance_exponent: 0.5,
            allow_resets: false,
            decrease_interval: TimeDelta::Millis(300),
            loss_report_timeout: TimeDelta::Millis(6000),
        }
    }
}

// Estimates an upper BWE limit based on loss.
// It requires knowledge about lost packets and acknowledged bitrate.
// Ie, this class require transport feedback.
pub struct LossBasedBandwidthEstimation {
    config: LossBasedControlConfig,
    average_loss: f64,
    average_loss_max: f64,
    loss_based_bitrate: DataRate,
    acknowledged_bitrate_max: DataRate,
    acknowledged_bitrate_last_update: Timestamp,
    time_last_decrease: Timestamp,
    has_decreased_since_last_loss_report: bool,
    last_loss_packet_report: Timestamp,
    last_loss_ratio: f64,
}

impl LossBasedBandwidthEstimation {
    pub fn new(config: LossBasedControlConfig) -> Self {
        Self {
            config,
            average_loss: 0.0,
            average_loss_max: 0.0,
            loss_based_bitrate: DataRate::Zero(),
            acknowledged_bitrate_max: DataRate::Zero(),
            acknowledged_bitrate_last_update: Timestamp::MinusInfinity(),
            time_last_decrease: Timestamp::MinusInfinity(),
            has_decreased_since_last_loss_report: false,
            last_loss_packet_report: Timestamp::MinusInfinity(),
            last_loss_ratio: 0.0,
        }
    }

    // Returns the new estimate.
    pub fn Update(
        &mut self,
        at_time: Timestamp,
        min_bitrate: DataRate,
        wanted_bitrate: DataRate,
        last_round_trip_time: TimeDelta,
    ) -> DataRate {
        todo!();
    }
    pub fn UpdateAcknowledgedBitrate(
        &mut self,
        acknowledged_bitrate: DataRate,
        at_time: Timestamp,
    ) {
        todo!();
    }
    pub fn Initialize(&mut self, bitrate: DataRate) {
        todo!();
    }
    pub fn Enabled(&self) -> bool {
        self.config.enabled
    }
    // Returns true if LossBasedBandwidthEstimation is enabled and have
    // received loss statistics. Ie, this class require transport feedback.
    pub fn InUse(&self) -> bool {
        self.Enabled() && self.last_loss_packet_report.IsFinite()
    }
    pub fn UpdateLossStatistics(&mut self, packet_results: &[PacketResult], at_time: Timestamp) {
        todo!();
    }
    pub fn GetEstimate(&self) -> DataRate {
        self.loss_based_bitrate
    }

    fn Reset(&mut self, bitrate: DataRate) {
        todo!();
    }
    fn loss_increase_threshold(&self) -> f64 {
        todo!();
    }
    fn loss_decrease_threshold(&self) -> f64 {
        todo!();
    }
    fn loss_reset_threshold(&self) -> f64 {
        todo!();
    }

    fn decreased_bitrate(&self) -> DataRate {
        todo!();
    }
}
