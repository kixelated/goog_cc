/*
 *  Copyright (c) 2017 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_BITRATE_ESTIMATOR_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_BITRATE_ESTIMATOR_H_

#include <stdint.h>

#include <optional>

#include "api/field_trials_view.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/timestamp.h"
#include "rtc_base/experiments/field_trial_parser.h"



// Computes a bayesian estimate of the throughput given acks containing
// the arrival time and payload size. Samples which are far from the current
// estimate or are based on few packets are given a smaller weight, as they
// are considered to be more likely to have been caused by, e.g., delay spikes
// unrelated to congestion.
pub struct BitrateEstimator {
 public:
  explicit BitrateEstimator(const FieldTrialsView* key_value_config);
  virtual ~BitrateEstimator();
  virtual void Update(Timestamp at_time, DataSize amount, bool in_alr);

  virtual Option<DataRate> bitrate() const;
  Option<DataRate> PeekRate() const;

  virtual void ExpectFastRateChange();

 private:
  float UpdateWindow(i64 now_ms,
                     int bytes,
                     int rate_window_ms,
                     bool* is_small_sample);
  sum: int,
  initial_window_ms: FieldTrialConstrained<int>,
  noninitial_window_ms: FieldTrialConstrained<int>,
  uncertainty_scale: FieldTrialParameter<f64>,
  uncertainty_scale_in_alr: FieldTrialParameter<f64>,
  small_sample_uncertainty_scale: FieldTrialParameter<f64>,
  small_sample_threshold: FieldTrialParameter<DataSize>,
  uncertainty_symmetry_cap: FieldTrialParameter<DataRate>,
  estimate_floor: FieldTrialParameter<DataRate>,
  current_window_ms: i64,
  prev_time_ms: i64,
  bitrate_estimate_kbps: float,
  bitrate_estimate_var: float,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_BITRATE_ESTIMATOR_H_
