/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/congestion_window_pushback_controller.h"

#include <algorithm>
#include <cstdint>

#include "api/field_trials_view.h"
#include "api/units/data_size.h"
#include "rtc_base/experiments/rate_control_settings.h"



CongestionWindowPushbackController::CongestionWindowPushbackController(
    const FieldTrialsView& key_value_config)
    : add_pacing_(key_value_config.IsEnabled(
          "WebRTC-AddPacingToCongestionWindowPushback")),
      min_pushback_target_bitrate_bps_(
          RateControlSettings(key_value_config)
              .CongestionWindowMinPushbackTargetBitrateBps()),
      current_data_window_(RateControlSettings(key_value_config)
                               .CongestionWindowInitialDataWindow()) {}

fn UpdateOutstandingData(&self /* CongestionWindowPushbackController */,
    i64 outstanding_bytes) {
  self.outstanding_bytes = outstanding_bytes;
}
fn UpdatePacingQueue(&self /* CongestionWindowPushbackController */,
    i64 pacing_bytes) {
  self.pacing_bytes = pacing_bytes;
}

fn SetDataWindow(&self /* CongestionWindowPushbackController */,DataSize data_window) {
  self.current_data_window = data_window;
}

u32 UpdateTargetBitrate(&self /* CongestionWindowPushbackController */,
    u32 bitrate_bps) {
  if (!self.current_data_window || self.current_data_window.IsZero())
    return bitrate_bps;
  let total_bytes: i64 = self.outstanding_bytes;
  if (self.add_pacing)
    total_bytes += self.pacing_bytes;
  let fill_ratio: f64 =
      total_bytes / (self.current_data_window.bytes() as f64);
  if (fill_ratio > 1.5) {
    self.encoding_rate_ratio *= 0.9;
  } else if (fill_ratio > 1) {
    self.encoding_rate_ratio *= 0.95;
  } else if (fill_ratio < 0.1) {
    self.encoding_rate_ratio = 1.0;
  } else {
    self.encoding_rate_ratio *= 1.05;
    self.encoding_rate_ratio = std::cmp::min(self.encoding_rate_ratio, 1.0);
  }
  let adjusted_target_bitrate_bps: u32 =
      (bitrate_bps * self.encoding_rate_ratio) as u32;

  // Do not adjust below the minimum pushback bitrate but do obey if the
  // original estimate is below it.
  bitrate_bps = adjusted_target_bitrate_bps < min_pushback_target_bitrate_bps_
                    ? std::cmp::min(bitrate_bps, self.min_pushback_target_bitrate_bps)
                    : adjusted_target_bitrate_bps;
  return bitrate_bps;
}

}  // namespace webrtc
