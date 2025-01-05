/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::{api::units::DataSize, FieldTrials};

// WebRTC-CongestionWindow
#[derive(Clone, Debug)]
pub struct CongestionWindowConfig {
    pub queue_size_ms: i64,                    // QueueSize
    pub min_bitrate_bps: i64,                  // MinBitrate
    pub initial_data_window: Option<DataSize>, // InitWin
    pub drop_frame_only: bool,                 // DropFrame
}

impl Default for CongestionWindowConfig {
    // QueueSize:350,MinBitrate:30000,DropFrame:true
    fn default() -> Self {
        Self {
            queue_size_ms: 350,
            min_bitrate_bps: 30000,
            initial_data_window: None,
            drop_frame_only: true,
        }
    }
}

// WebRTC-VideoRateControl
#[derive(Clone, Debug)]
pub struct VideoRateControlConfig {
    pub pacing_factor: Option<f64>,
    pub alr_probing: bool,
    pub vp8_qp_max: Option<i64>,
    pub vp8_min_pixels: Option<i64>,
    pub trust_vp8: bool,             // = true
    pub trust_vp9: bool,             // = true;
    pub bitrate_adjuster: bool,      // = true;
    pub adjuster_use_headroom: bool, // = true;
    pub vp8_s0_boost: bool,
    pub vp8_base_heavy_tl3_alloc: bool,
}

impl Default for VideoRateControlConfig {
    fn default() -> Self {
        Self {
            pacing_factor: None,
            alr_probing: false,
            vp8_qp_max: None,
            vp8_min_pixels: None,
            trust_vp8: true,
            trust_vp9: true,
            bitrate_adjuster: true,
            adjuster_use_headroom: true,
            vp8_s0_boost: false,
            vp8_base_heavy_tl3_alloc: false,
        }
    }
}

#[derive(Default)]
pub struct RateControlSettings {
    congestion_window_config: CongestionWindowConfig,
    video_config: VideoRateControlConfig,
}

impl RateControlSettings {
    const DefaultAcceptedQueueMs: i64 = 350;
    const DefaultMinPushbackTargetBitrateBps: i64 = 30000;

    // const char kCongestionWindowDefaultFieldTrialString[] =
    //"QueueSize:350,MinBitrate:30000,DropFrame:true";
    //const char kUseBaseHeavyVp8Tl3RateAllocationFieldTrialName[] =
    //   "WebRTC-UseBaseHeavyVP8TL3RateAllocation";

    pub fn new(field_trials: &FieldTrials) -> Self {
        let congestion_window_config = field_trials.congestion_window.clone();
        let mut video_config = field_trials.video_rate_control.clone();
        video_config.vp8_base_heavy_tl3_alloc =
            video_config.vp8_base_heavy_tl3_alloc || field_trials.vp8_base_heavy_tl3_alloc;
        Self {
            congestion_window_config,
            video_config,
        }
    }

    // When CongestionWindowPushback is enabled, the pacer is oblivious to
    // the congestion window. The relation between outstanding data and
    // the congestion window affects encoder allocations directly.
    pub fn UseCongestionWindow(&self) -> bool {
        self.congestion_window_config.queue_size_ms > 0
    }
    pub fn GetCongestionWindowAdditionalTimeMs(&self) -> i64 {
        self.congestion_window_config.queue_size_ms
    }
    pub fn UseCongestionWindowPushback(&self) -> bool {
        return self.congestion_window_config.queue_size_ms > 0
            && self.congestion_window_config.min_bitrate_bps > 0;
    }
    pub fn UseCongestionWindowDropFrameOnly(&self) -> bool {
        return self.congestion_window_config.drop_frame_only;
    }
    pub fn CongestionWindowMinPushbackTargetBitrateBps(&self) -> u32 {
        return self.congestion_window_config.min_bitrate_bps as _;
    }
    pub fn CongestionWindowInitialDataWindow(&self) -> Option<DataSize> {
        return self.congestion_window_config.initial_data_window;
    }

    pub fn GetPacingFactor(&self) -> Option<f64> {
        return self.video_config.pacing_factor;
    }
    pub fn UseAlrProbing(&self) -> bool {
        return self.video_config.alr_probing;
    }

    pub fn LibvpxVp8QpMax(&self) -> Option<i64> {
        if let Some(vp8_qp_max) = self.video_config.vp8_qp_max {
            if vp8_qp_max < 0 || vp8_qp_max > 63 {
                tracing::warn!("Unsupported vp8_qp_max_ value, ignored.");
                return None;
            }
        }
        return self.video_config.vp8_qp_max;
    }
    pub fn LibvpxVp8MinPixels(&self) -> Option<i64> {
        if let Some(vp8_min_pixels) = self.video_config.vp8_min_pixels {
            if vp8_min_pixels < 1 {
                tracing::warn!("Unsupported vp8_min_pixels_ value, ignored.");
                return None;
            }
        }
        return self.video_config.vp8_min_pixels;
    }
    pub fn LibvpxVp8TrustedRateController(&self) -> bool {
        return self.video_config.trust_vp8;
    }
    pub fn Vp8BoostBaseLayerQuality(&self) -> bool {
        return self.video_config.vp8_s0_boost;
    }
    pub fn Vp8DynamicRateSettings(&self) -> bool {
        todo!();
    }
    pub fn LibvpxVp9TrustedRateController(&self) -> bool {
        return self.video_config.trust_vp9;
    }
    pub fn Vp9DynamicRateSettings(&self) -> bool {
        todo!();
    }

    pub fn Vp8BaseHeavyTl3RateAllocation(&self) -> bool {
        return self.video_config.vp8_base_heavy_tl3_alloc;
    }

    pub fn UseEncoderBitrateAdjuster(&self) -> bool {
        return self.video_config.bitrate_adjuster;
    }
    pub fn BitrateAdjusterCanUseNetworkHeadroom(&self) -> bool {
        return self.video_config.adjuster_use_headroom;
    }
}
