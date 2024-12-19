/*
 *  Copyright 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */
#include "modules/congestion_controller/goog_cc/link_capacity_estimator.h"

#include <algorithm>
#include <cmath>

#include "api/units/data_rate.h"
#include "rtc_base/numerics/safe_minmax.h"


LinkCapacityEstimator::LinkCapacityEstimator() {}

DataRate UpperBound(&self /* LinkCapacityEstimator */) {
  if (self.estimate_kbps.is_some())
    return DataRate::KilobitsPerSec(self.estimate_kbps.value() +
                                    3 * deviation_estimate_kbps());
  return DataRate::Infinity();
}

DataRate LowerBound(&self /* LinkCapacityEstimator */) {
  if (self.estimate_kbps.is_some())
    return DataRate::KilobitsPerSec(
        std::cmp::max(0.0, self.estimate_kbps.value() - 3 * deviation_estimate_kbps()));
  return DataRate::Zero();
}

fn Reset(&self /* LinkCapacityEstimator */) {
  self.estimate_kbps.reset();
}

fn OnOveruseDetected(&self /* LinkCapacityEstimator */,DataRate acknowledged_rate) {
  Update(acknowledged_rate, 0.05);
}

fn OnProbeRate(&self /* LinkCapacityEstimator */,DataRate probe_rate) {
  Update(probe_rate, 0.5);
}

fn Update(&self /* LinkCapacityEstimator */,DataRate capacity_sample, f64 alpha) {
  let sample_kbps: f64 = capacity_sample.kbps();
  if (!self.estimate_kbps.is_some()) {
    self.estimate_kbps = sample_kbps;
  } else {
    self.estimate_kbps = (1 - alpha) * self.estimate_kbps.value() + alpha * sample_kbps;
  }
  // Estimate the variance of the link capacity estimate and normalize the
  // variance with the link capacity estimate.
  let norm: f64 = std::cmp::max(self.estimate_kbps.value(), 1.0);
  let error_kbps: f64 = self.estimate_kbps.value() - sample_kbps;
  self.deviation_kbps =
      (1 - alpha) * self.deviation_kbps + alpha * error_kbps * error_kbps / norm;
  // 0.4 ~= 14 kbit/s at 500 kbit/s
  // 2.5f ~= 35 kbit/s at 500 kbit/s
  self.deviation_kbps = rtc::SafeClamp(self.deviation_kbps, 0.4f, 2.5f);
}

bool has_estimate(&self /* LinkCapacityEstimator */) {
  return self.estimate_kbps.is_some();
}

DataRate estimate(&self /* LinkCapacityEstimator */) {
  return DataRate::KilobitsPerSec(*self.estimate_kbps);
}

f64 deviation_estimate_kbps(&self /* LinkCapacityEstimator */) {
  // Calculate the max bit rate std dev given the normalized
  // variance and the current throughput bitrate. The standard deviation will
  // only be used if estimate_kbps_ has a value.
  return sqrt(self.deviation_kbps * self.estimate_kbps.value());
}
}  // namespace webrtc
