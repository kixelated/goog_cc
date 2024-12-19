/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_CONGESTION_WINDOW_PUSHBACK_CONTROLLER_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_CONGESTION_WINDOW_PUSHBACK_CONTROLLER_H_

#include <stdint.h>

#include <optional>

#include "api/field_trials_view.h"
#include "api/units/data_size.h"



// This class enables pushback from congestion window directly to video encoder.
// When the congestion window is filling up, the video encoder target bitrate
// will be reduced accordingly to accommodate the network changes. To avoid
// pausing video too frequently, a minimum encoder target bitrate threshold is
// used to prevent video pause due to a full congestion window.
pub struct CongestionWindowPushbackController {
 public:
  explicit CongestionWindowPushbackController(
      const FieldTrialsView& key_value_config);
  fn UpdateOutstandingData(i64 outstanding_bytes) {
  todo!();
}
  fn UpdatePacingQueue(i64 pacing_bytes) {
  todo!();
}
  u32 UpdateTargetBitrate(u32 bitrate_bps);
  fn SetDataWindow(DataSize data_window) {
  todo!();
}

 private:
  const bool self.add_pacing;
  const u32 self.min_pushback_target_bitrate_bps;
  current_data_window: Option<DataSize>,
  i64 self.outstanding_bytes = 0;
  i64 self.pacing_bytes = 0;
  let encoding_rate_ratio_: f64 = 1.0;
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_CONGESTION_WINDOW_PUSHBACK_CONTROLLER_H_
