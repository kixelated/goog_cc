/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::{api::units::DataSize, experiments::RateControlSettings, FieldTrials};

// This class enables pushback from congestion window directly to video encoder.
// When the congestion window is filling up, the video encoder target bitrate
// will be reduced accordingly to accommodate the network changes. To avoid
// pausing video too frequently, a minimum encoder target bitrate threshold is
// used to prevent video pause due to a full congestion window.
pub struct CongestionWindowPushbackController {
    add_pacing: bool,
    min_pushback_target_bitrate_bps: u32,
    current_data_window: Option<DataSize>,
    outstanding_bytes: i64,
    pacing_bytes: i64,
    encoding_rate_ratio: f64,
}

// const char CongestionWindowDefaultFieldTrialString[] = "QueueSize:350,MinBitrate:30000,DropFrame:true"
impl Default for CongestionWindowPushbackController {
    fn default() -> Self {
        Self {
            add_pacing: false,
            min_pushback_target_bitrate_bps: 30000,
            current_data_window: None,
            outstanding_bytes: 0,
            pacing_bytes: 0,
            encoding_rate_ratio: 1.0,
        }
    }
}

impl CongestionWindowPushbackController {
    pub fn new(field_trials: &FieldTrials) -> Self {
        let rate_control_settings = RateControlSettings::new(field_trials);
        Self {
            add_pacing: field_trials.add_pacing_to_congestion_window_pushback,
            min_pushback_target_bitrate_bps: rate_control_settings.CongestionWindowMinPushbackTargetBitrateBps(),
            current_data_window: rate_control_settings.CongestionWindowInitialDataWindow(),
            outstanding_bytes: 0,
            pacing_bytes: 0,
            encoding_rate_ratio: 1.0,
        }
    }

    pub fn UpdateOutstandingData(&mut self, outstanding_bytes: i64) {
        self.outstanding_bytes = outstanding_bytes;
    }
    pub fn UpdatePacingQueue(&mut self, pacing_bytes: i64) {
        self.pacing_bytes = pacing_bytes;
    }
    pub fn UpdateTargetBitrate(&mut self, mut bitrate_bps: u32) -> u32 {
        let data_window = match self.current_data_window {
            Some(data_window) if !data_window.IsZero() => data_window,
            _ => return bitrate_bps,
        };
        let mut total_bytes: i64 = self.outstanding_bytes;
        if self.add_pacing {
            total_bytes += self.pacing_bytes;
        }
        let fill_ratio: f64 = total_bytes as f64 / (data_window.bytes() as f64);
        if fill_ratio > 1.5 {
            self.encoding_rate_ratio *= 0.9;
        } else if fill_ratio > 1.0 {
            self.encoding_rate_ratio *= 0.95;
        } else if fill_ratio < 0.1 {
            self.encoding_rate_ratio = 1.0;
        } else {
            self.encoding_rate_ratio *= 1.05;
            self.encoding_rate_ratio = self.encoding_rate_ratio.min(1.0);
        }
        let adjusted_target_bitrate_bps: u32 =
            (bitrate_bps as f64 * self.encoding_rate_ratio) as u32;

        // Do not adjust below the minimum pushback bitrate but do obey if the
        // original estimate is below it.
        bitrate_bps = if adjusted_target_bitrate_bps < self.min_pushback_target_bitrate_bps {
            std::cmp::min(bitrate_bps, self.min_pushback_target_bitrate_bps)
        } else {
            adjusted_target_bitrate_bps
        };
        bitrate_bps
    }

    pub fn SetDataWindow(&mut self, data_window: DataSize) {
        self.current_data_window = Some(data_window);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn FullCongestionWindow() {
        let mut cwnd_controller = CongestionWindowPushbackController::default();

        cwnd_controller.UpdateOutstandingData(100000);
        cwnd_controller.SetDataWindow(DataSize::Bytes(50000));

        let mut bitrate_bps: u32 = 80000;
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert_eq!(72000, bitrate_bps);

        cwnd_controller.SetDataWindow(DataSize::Bytes(50000));
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert_eq!((72000.0 * 0.9 * 0.9) as u32, bitrate_bps);
    }

    #[test]
    fn NormalCongestionWindow() {
        let mut cwnd_controller = CongestionWindowPushbackController::default();

        cwnd_controller.UpdateOutstandingData(199999);
        cwnd_controller.SetDataWindow(DataSize::Bytes(200000));

        let mut bitrate_bps: u32 = 80000;
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert_eq!(80000, bitrate_bps);
    }

    #[test]
    fn LowBitrate() {
        let mut cwnd_controller = CongestionWindowPushbackController::default();

        cwnd_controller.UpdateOutstandingData(100000);
        cwnd_controller.SetDataWindow(DataSize::Bytes(50000));

        let mut bitrate_bps: u32 = 35000;
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert_eq!((35000.0 * 0.9) as u32, bitrate_bps);

        cwnd_controller.SetDataWindow(DataSize::Bytes(20000));
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert_eq!(30000, bitrate_bps);
    }

    #[test]
    fn NoPushbackOnDataWindowUnset() {
        let mut cwnd_controller = CongestionWindowPushbackController::default();

        cwnd_controller.UpdateOutstandingData(100_000_000); // Large number

        let mut bitrate_bps: u32 = 80000;
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert_eq!(80000, bitrate_bps);
    }

    #[test]
    fn PushbackOnInititialDataWindow() {
        let mut cwnd_controller = CongestionWindowPushbackController::default();
        cwnd_controller.SetDataWindow(DataSize::Bytes(100000));
        cwnd_controller.UpdateOutstandingData(100_000_000); // Large number

        let mut bitrate_bps: u32 = 80000;
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert!(80000 > bitrate_bps);
    }

    #[test]
    fn PushbackDropFrame() {
        let mut cwnd_controller = CongestionWindowPushbackController::default();
        // Not sure what this is meant to test.  The DropFrame field is not used.
        //CongestionWindowPushbackController cwnd_controller(
        //    ExplicitKeyValueConfig("WebRTC-CongestionWindow/DropFrame:true/"));

        cwnd_controller.UpdateOutstandingData(100_000_000); // Large number
        cwnd_controller.SetDataWindow(DataSize::Bytes(50000));

        let mut bitrate_bps: u32 = 80000;
        bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
        assert!(80000 > bitrate_bps);
    }
}
