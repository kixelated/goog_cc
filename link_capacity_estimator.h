/*
 *  Copyright 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */
#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_LINK_CAPACITY_ESTIMATOR_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_LINK_CAPACITY_ESTIMATOR_H_

#include <optional>

#include "api/units/data_rate.h"


pub struct LinkCapacityEstimator {
 public:
  LinkCapacityEstimator();
  DataRate UpperBound() const;
  DataRate LowerBound() const;
  void Reset();
  fn OnOveruseDetected(DataRate acknowledged_rate) {
  todo!();
}
  fn OnProbeRate(DataRate probe_rate) {
  todo!();
}
  bool has_estimate() const;
  DataRate estimate() const;

 private:
  friend class GoogCcStatePrinter;
  fn Update(DataRate capacity_sample, f64 alpha) {
  todo!();
}

  f64 deviation_estimate_kbps() const;
  estimate_kbps: Option<f64>,
  let deviation_kbps_: f64 = 0.4;
};
}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_LINK_CAPACITY_ESTIMATOR_H_
