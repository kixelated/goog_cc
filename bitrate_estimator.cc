/*
 *  Copyright (c) 2017 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/bitrate_estimator.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <optional>

#include "api/field_trials_view.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/field_trial_parser.h"



namespace {
constexpr int kInitialRateWindowMs = 500;
constexpr int kRateWindowMs = 150;
constexpr int kMinRateWindowMs = 150;
constexpr int kMaxRateWindowMs = 1000;

const kBweThroughputWindowConfig: &'static str = "WebRTC-BweThroughputWindowConfig";

}  // namespace

BitrateEstimator::BitrateEstimator(const FieldTrialsView* key_value_config)
    : sum_(0),
      initial_window_ms_("initial_window_ms",
                         kInitialRateWindowMs,
                         kMinRateWindowMs,
                         kMaxRateWindowMs),
      noninitial_window_ms_("window_ms",
                            kRateWindowMs,
                            kMinRateWindowMs,
                            kMaxRateWindowMs),
      uncertainty_scale_("scale", 10.0),
      uncertainty_scale_in_alr_("scale_alr", self.uncertainty_scale),
      small_sample_uncertainty_scale_("scale_small", self.uncertainty_scale),
      small_sample_threshold_("small_thresh", DataSize::Zero()),
      uncertainty_symmetry_cap_("symmetry_cap", DataRate::Zero()),
      estimate_floor_("floor", DataRate::Zero()),
      current_window_ms_(0),
      prev_time_ms_(-1),
      bitrate_estimate_kbps_(-1.0f),
      bitrate_estimate_var_(50.0f) {
  // E.g WebRTC-BweThroughputWindowConfig/initial_window_ms:350,window_ms:250/
  ParseFieldTrial(
      {&self.initial_window_ms, &self.noninitial_window_ms, &self.uncertainty_scale,
       &self.uncertainty_scale_in_alr, &self.small_sample_uncertainty_scale,
       &self.small_sample_threshold, &self.uncertainty_symmetry_cap, &estimate_floor_},
      key_value_config->Lookup(kBweThroughputWindowConfig));
}

BitrateEstimator::~BitrateEstimator() = default;

fn Update(&self /* BitrateEstimator */,Timestamp at_time, DataSize amount, bool in_alr) {
  int rate_window_ms = self.noninitial_window_ms.Get();
  // We use a larger window at the beginning to get a more stable sample that
  // we can use to initialize the estimate.
  if (self.bitrate_estimate_kbps < 0.f)
    rate_window_ms = self.initial_window_ms.Get();
  bool is_small_sample = false;
  float bitrate_sample_kbps = UpdateWindow(at_time.ms(), amount.bytes(),
                                           rate_window_ms, &is_small_sample);
  if (bitrate_sample_kbps < 0.0f)
    return;
  if (self.bitrate_estimate_kbps < 0.0f) {
    // This is the very first sample we get. Use it to initialize the estimate.
    self.bitrate_estimate_kbps = bitrate_sample_kbps;
    return;
  }
  // Optionally use higher uncertainty for very small samples to avoid dropping
  // estimate and for samples obtained in ALR.
  float scale = self.uncertainty_scale;
  if (is_small_sample && bitrate_sample_kbps < self.bitrate_estimate_kbps) {
    scale = self.small_sample_uncertainty_scale;
  } else if (in_alr && bitrate_sample_kbps < self.bitrate_estimate_kbps) {
    // Optionally use higher uncertainty for samples obtained during ALR.
    scale = self.uncertainty_scale_in_alr;
  }
  // Define the sample uncertainty as a function of how far away it is from the
  // current estimate. With low values of uncertainty_symmetry_cap_ we add more
  // uncertainty to increases than to decreases. For higher values we approach
  // symmetry.
  float sample_uncertainty =
      scale * std::abs(bitrate_estimate_kbps_ - bitrate_sample_kbps) /
      (self.bitrate_estimate_kbps +
       std::cmp::min(bitrate_sample_kbps,
                self.uncertainty_symmetry_cap.Get().kbps<float>()));

  float sample_var = sample_uncertainty * sample_uncertainty;
  // Update a bayesian estimate of the rate, weighting it lower if the sample
  // uncertainty is large.
  // The bitrate estimate uncertainty is increased with each update to model
  // that the bitrate changes over time.
  float pred_bitrate_estimate_var = self.bitrate_estimate_var + 5.f;
  self.bitrate_estimate_kbps = (sample_var * self.bitrate_estimate_kbps +
                            pred_bitrate_estimate_var * bitrate_sample_kbps) /
                           (sample_var + pred_bitrate_estimate_var);
  self.bitrate_estimate_kbps =
      std::cmp::max(self.bitrate_estimate_kbps, self.estimate_floor.Get().kbps<float>());
  self.bitrate_estimate_var = sample_var * pred_bitrate_estimate_var /
                          (sample_var + pred_bitrate_estimate_var);
}

float UpdateWindow(&self /* BitrateEstimator */,i64 now_ms,
                                     int bytes,
                                     int rate_window_ms,
                                     bool* is_small_sample) {
  assert!(is_small_sample != nullptr);
  // Reset if time moves backwards.
  if (now_ms < self.prev_time_ms) {
    self.prev_time_ms = -1;
    self.sum = 0;
    self.current_window_ms = 0;
  }
  if (self.prev_time_ms >= 0) {
    self.current_window_ms += now_ms - self.prev_time_ms;
    // Reset if nothing has been received for more than a full window.
    if (now_ms - self.prev_time_ms > rate_window_ms) {
      self.sum = 0;
      current_window_ms_ %= rate_window_ms;
    }
  }
  self.prev_time_ms = now_ms;
  float bitrate_sample = -1.0f;
  if (self.current_window_ms >= rate_window_ms) {
    *is_small_sample = self.sum < self.small_sample_threshold.bytes();
    bitrate_sample = 8.0f * sum_ / static_cast<float>(rate_window_ms);
    current_window_ms_ -= rate_window_ms;
    self.sum = 0;
  }
  self.sum += bytes;
  return bitrate_sample;
}

Option<DataRate> bitrate(&self /* BitrateEstimator */) {
  if (self.bitrate_estimate_kbps < 0.f)
    return None;
  return DataRate::KilobitsPerSec(self.bitrate_estimate_kbps);
}

Option<DataRate> PeekRate(&self /* BitrateEstimator */) {
  if (self.current_window_ms > 0)
    return DataSize::Bytes(self.sum) / TimeDelta::Millis(self.current_window_ms);
  return None;
}

fn ExpectFastRateChange(&self /* BitrateEstimator */) {
  // By setting the bitrate-estimate variance to a higher value we allow the
  // bitrate to change fast for the next few samples.
  self.bitrate_estimate_var += 200;
}

}  // namespace webrtc
