/*
 *  Copyright (c) 2017 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::api::units::{DataRate, DataSize, TimeDelta};


// Computes a bayesian estimate of the throughput given acks containing
// the arrival time and payload size. Samples which are far from the current
// estimate or are based on few packets are given a smaller weight, as they
// are considered to be more likely to have been caused by, e.g., delay spikes
// unrelated to congestion.
pub struct BitrateEstimator {
  sum: isize,
  initial_window_ms: isize,
  noninitial_window_ms: isize,
  uncertainty_scale: f64,
  uncertainty_scale_in_alr: f64,
  small_sample_uncertainty_scale: f64,
  small_sample_threshold: DataSize,
  uncertainty_symmetry_cap: DataRate,
  estimate_floor: DataRate,
  current_window_ms: i64,
  prev_time_ms: i64,
  bitrate_estimate_kbps: f32,
  bitrate_estimate_var: f32,
}

impl Default for BitrateEstimator {
  fn default() -> Self {
    Self {
      sum: 0,
      initial_window_ms: InitialRateWindowMs,
      noninitial_window_ms: RateWindowMs,
      uncertainty_scale: 10.0,
      uncertainty_scale_in_alr: 10.0,
      small_sample_uncertainty_scale: 10.0,
      small_sample_threshold: DataSize::Zero(),
      uncertainty_symmetry_cap: DataRate::Zero(),
      estimate_floor: DataRate::Zero(),
      current_window_ms: 0,
      prev_time_ms: -1,
      bitrate_estimate_kbps: -1.0,
      bitrate_estimate_var: 50.0,
    }
  }
}


 const InitialRateWindowMs: isize = 500;
 const RateWindowMs: isize = 150;
 const MinRateWindowMs: isize = 150;
 const MaxRateWindowMs: isize = 1000;

impl BitrateEstimator {
  pub fn Update(&mut self, at_time: Timestamp, amount: DataSize, in_alr: bool) {
   let rate_window_ms: isize = self.noninitial_window_ms;
   // We use a larger window at the beginning to get a more stable sample that
   // we can use to initialize the estimate.
   if (self.bitrate_estimate_kbps < 0.0) {
     rate_window_ms = self.initial_window_ms;
   }
   let mut is_small_sample: bool = false;
   let bitrate_sample_kbps: f32 = self.UpdateWindow(at_time.ms(), amount.bytes(),
                                            rate_window_ms, &mut is_small_sample);
   if (bitrate_sample_kbps < 0.0) {
     return;
   }
   if (self.bitrate_estimate_kbps < 0.0) {
     // This is the very first sample we get. Use it to initialize the estimate.
     self.bitrate_estimate_kbps = bitrate_sample_kbps;
     return;
   }
   // Optionally use higher uncertainty for very small samples to avoid dropping
   // estimate and for samples obtained in ALR.
   let mut scale: f32 = self.uncertainty_scale as f32;
   if (is_small_sample && bitrate_sample_kbps < self.bitrate_estimate_kbps) {
     scale = self.small_sample_uncertainty_scale as f32;
   } else if (in_alr && bitrate_sample_kbps < self.bitrate_estimate_kbps) {
     // Optionally use higher uncertainty for samples obtained during ALR.
     scale = self.uncertainty_scale_in_alr as f32;
   }
   // Define the sample uncertainty as a function of how far away it is from the
   // current estimate. With low values of uncertainty_symmetry_cap_ we add more
   // uncertainty to increases than to decreases. For higher values we approach
   // symmetry.
   let sample_uncertainty: f32 =
       scale * (self.bitrate_estimate_kbps - bitrate_sample_kbps).abs() /
       (self.bitrate_estimate_kbps +
        std::cmp::min(bitrate_sample_kbps,
                 self.uncertainty_symmetry_cap.kbps::<f32>()));

   let sample_var: f32 = sample_uncertainty * sample_uncertainty;
   // Update a bayesian estimate of the rate, weighting it lower if the sample
   // uncertainty is large.
   // The bitrate estimate uncertainty is increased with each update to model
   // that the bitrate changes over time.
   let pred_bitrate_estimate_var: f32 = self.bitrate_estimate_var + 5.0;
   self.bitrate_estimate_kbps = (sample_var * self.bitrate_estimate_kbps +
                             pred_bitrate_estimate_var * bitrate_sample_kbps) /
                            (sample_var + pred_bitrate_estimate_var);
   self.bitrate_estimate_kbps =
       std::cmp::max(self.bitrate_estimate_kbps, self.estimate_floor.kbps::<f32>());
   self.bitrate_estimate_var = sample_var * pred_bitrate_estimate_var /
                           (sample_var + pred_bitrate_estimate_var);
  }

  pub fn bitrate(&self) -> Option<DataRate> {
   if (self.bitrate_estimate_kbps < 0.0) {
     return None;
   }
   return Some(DataRate::KilobitsPerSec(self.bitrate_estimate_kbps));
  }

  pub fn PeekRate(&self) -> Option<DataRate> {
   if (self.current_window_ms > 0) {
     return Some(DataSize::Bytes(self.sum) / TimeDelta::Millis(self.current_window_ms));
   }
   return None;
  }

  pub fn ExpectFastRateChange(&mut self) {
   // By setting the bitrate-estimate variance to a higher value we allow the
   // bitrate to change fast for the next few samples.
   self.bitrate_estimate_var += 200;

  }

  fn UpdateWindow(&self, now_ms: i64, bytes: isize, rate_window_ms: isize, is_small_sample: &mut bool) -> f32 {
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
       self.current_window_ms %= rate_window_ms;
     }
   }
   self.prev_time_ms = now_ms;
   let bitrate_sample: f32 = -1.0;
   if (self.current_window_ms >= rate_window_ms) {
     *is_small_sample = self.sum < self.small_sample_threshold.bytes();
     bitrate_sample = 8.0 * self.sum / (rate_window_ms) as f32;
     self.current_window_ms -= rate_window_ms;
     self.sum = 0;
   }
   self.sum += bytes;
   return bitrate_sample;
  }
}