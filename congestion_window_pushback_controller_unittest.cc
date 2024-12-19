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

#include <cstdint>

#include "api/units/data_size.h"
#include "test/explicit_key_value_config.h"
#include "test/gmock.h"
#include "test/gtest.h"


namespace test {

#[test]
fn FullCongestionWindow() {
  CongestionWindowPushbackController cwnd_controller(
      ExplicitKeyValueConfig(""));

  cwnd_controller.UpdateOutstandingData(100000);
  cwnd_controller.SetDataWindow(DataSize::Bytes(50000));

  let bitrate_bps: u32 = 80000;
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  assert_eq!(72000u, bitrate_bps);

  cwnd_controller.SetDataWindow(DataSize::Bytes(50000));
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  assert_eq!((72000 * 0.9 * 0.9) as u32, bitrate_bps);
}

#[test]
fn NormalCongestionWindow() {
  CongestionWindowPushbackController cwnd_controller(
      ExplicitKeyValueConfig(""));

  cwnd_controller.UpdateOutstandingData(199999);
  cwnd_controller.SetDataWindow(DataSize::Bytes(200000));

  let bitrate_bps: u32 = 80000;
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  assert_eq!(80000u, bitrate_bps);
}

#[test]
fn LowBitrate() {
  CongestionWindowPushbackController cwnd_controller(
      ExplicitKeyValueConfig(""));

  cwnd_controller.UpdateOutstandingData(100000);
  cwnd_controller.SetDataWindow(DataSize::Bytes(50000));

  let bitrate_bps: u32 = 35000;
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  assert_eq!((35000 * 0.9) as u32, bitrate_bps);

  cwnd_controller.SetDataWindow(DataSize::Bytes(20000));
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  assert_eq!(30000u, bitrate_bps);
}

#[test]
fn NoPushbackOnDataWindowUnset() {
  CongestionWindowPushbackController cwnd_controller(
      ExplicitKeyValueConfig(""));

  cwnd_controller.UpdateOutstandingData(1e8);  // Large number

  let bitrate_bps: u32 = 80000;
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  assert_eq!(80000u, bitrate_bps);
}

#[test]
fn PushbackOnInititialDataWindow() {
  CongestionWindowPushbackController cwnd_controller(
      ExplicitKeyValueConfig("WebRTC-CongestionWindow/InitWin:100000/"));

  cwnd_controller.UpdateOutstandingData(1e8);  // Large number

  let bitrate_bps: u32 = 80000;
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  EXPECT_GT(80000u, bitrate_bps);
}

#[test]
fn PushbackDropFrame() {
  CongestionWindowPushbackController cwnd_controller(
      ExplicitKeyValueConfig("WebRTC-CongestionWindow/DropFrame:true/"));

  cwnd_controller.UpdateOutstandingData(1e8);  // Large number
  cwnd_controller.SetDataWindow(DataSize::Bytes(50000));

  let bitrate_bps: u32 = 80000;
  bitrate_bps = cwnd_controller.UpdateTargetBitrate(bitrate_bps);
  EXPECT_GT(80000u, bitrate_bps);
}

}  // namespace test
}  // namespace webrtc
