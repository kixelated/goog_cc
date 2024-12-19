/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::VecDeque;

use crate::{api::transport::BandwidthUsage, DelayIncreaseDetectorInterface};

struct TrendlineEstimatorSettings {
  // Sort the packets in the window. Should be redundant,
  // but then almost no cost.
  pub enable_sort: bool,

  // Cap the trendline slope based on the minimum delay seen
  // in the beginning_packets and end_packets respectively.
  pub enable_cap: bool,
  pub beginning_packets: u64,
  pub end_packets: u64,
  pub cap_uncertainty: f64,

  // Size (in packets) of the window.
  pub window_size: u64,
}

impl Default for TrendlineEstimatorSettings {
  fn default() -> Self {
      Self {
        enable_sort: false,
        enable_cap: false,
        beginning_packets: 7,
        end_packets: 7,
        cap_uncertainty: 0.0,
        window_size: 20,
      }
  }
}


pub struct TrendlineEstimator {
  // Parameters.
  settings: TrendlineEstimatorSettings,
  smoothing_coef: f64,
  threshold_gain: f64,
  // Used by the existing threshold.
  num_of_deltas: isize,
  // Keep the arrival times small by using the change from the first packet.
  first_arrival_time_ms: i64,
  // Exponential backoff filtering.
  accumulated_delay: f64,
  smoothed_delay: f64,
  // Linear least squares regression.
  delay_hist: VecDeque<PacketTiming>,

  k_up: f64,
  k_down: f64,
  overusing_time_threshold: f64,
  threshold: f64,
  prev_modified_trend: f64,
  last_update_ms: i64,
  prev_trend: f64,
  time_over_using: f64,
  overuse_counter: isize,
  hypothesis: BandwidthUsage,
  hypothesis_predicted: BandwidthUsage,
  network_state_predictor: NetworkStatePredictor,
}

struct PacketTiming {
  pub arrival_time_ms: f64,
  pub smoothed_delay_ms: f64,
  pub raw_delay_ms: f64,
}

impl TrendlineEstimator {
  // Parameters for linear least squares fit of regression line to noisy data.
  const kDefaultTrendlineSmoothingCoeff: f64 = 0.9;
  const kDefaultTrendlineThresholdGain: f64 = 4.0;
  const kBweWindowSizeInPacketsExperiment: &'static str = "WebRTC-BweWindowSizeInPackets";

  pub fn new(network_state_predictor: NetworkStatePredictor) {

  }

  // Update the estimator with a new sample. The deltas should represent deltas
  // between timestamp groups as defined by the InterArrival class.
  pub fn Update(recv_delta_ms: f64,
              send_delta_ms: f64,
              send_time_ms: i64,
              arrival_time_ms: i64,
              packet_size: usize,
              calculated_deltas: bool) {
                todo!();
              }

  pub fn UpdateTrendline(recv_delta_ms: f64,
                       send_delta_ms: f64,
                       send_time_ms: i64,
                       arrival_time_ms: i64,
                       packet_size:usize) {
                          todo!();
                        }

  pub fn State() -> BandwidthUsage {
    todo!();
  }


  fn Detect(trend: f64, ts_delta: f64, now_ms: i64) {
  todo!();
}

  fn UpdateThreshold(modified_trend: f64, now_ms: i64) {
  todo!();
}
}

impl DelayIncreaseDetectorInterface for TrendlineEstimator {

  fn update(&mut self, recv_delta_ms: f64,
                        send_delta_ms: f64,
                        send_time_ms: i64,
                        arrival_time_ms: i64,
                        packet_size: usize,
                        calculated_deltas: bool) {
  if (calculated_deltas) {
    self.UpdateTrendline(recv_delta_ms, send_delta_ms, send_time_ms, arrival_time_ms,
                    packet_size);
  }
  if (self.network_state_predictor) {
    self.hypothesis_predicted = self.network_state_predictor.Update(
        send_time_ms, arrival_time_ms, self.hypothesis);
  }
}
  fn state(&self) -> BandwidthUsage {
    if self.network_state_predictor.is_some() {
      return self.hypothesis_predicted;
    } else {
      return self.hypothesis;
    }
  }
}

fn LinearFitSlope(
    packets: &VecDeque<TrendlineEstimator::PacketTiming>) -> Option<f64> {
  assert!(packets.len() >= 2);
  // Compute the "center of mass".
  let sum_x: f64 = 0.0;
  let sum_y : f64= 0.0;
  for packet in packets {
    sum_x += packet.arrival_time_ms;
    sum_y += packet.smoothed_delay_ms;
  }
  let x_avg: f64 = sum_x / packets.len();
  let y_avg: f64 = sum_y / packets.len();
  // Compute the slope k = \sum (x_i-x_avg)(y_i-y_avg) / \sum (x_i-x_avg)^2
  let numerator: f64 = 0.0;
  let denominator: f64 = 0.0;
  for packet in &packets {
    let x: f64 = packet.arrival_time_ms;
    let y: f64 = packet.smoothed_delay_ms;
    numerator += (x - x_avg) * (y - y_avg);
    denominator += (x - x_avg) * (x - x_avg);
  }
  if (denominator == 0.0) {
    return None
  }
  Some(numerator / denominator)
}

fn ComputeSlopeCap(
    packets: &VecDeque<TrendlineEstimator::PacketTiming>,
    settings: &TrendlineEstimatorSettings) -> Option<f64> {
  assert!(1 <= settings.beginning_packets &&
             settings.beginning_packets < packets.len());
  assert!(1 <= settings.end_packets &&
             settings.end_packets < packets.len());
  assert!(settings.beginning_packets + settings.end_packets <=
             packets.len());
  let early: PacketTiming = packets[0];
  for i in 1..settings.beginning_packets {
    if (packets[i].raw_delay_ms < early.raw_delay_ms) {
      early = packets[i];
    }
  }
  let late_start: usize = packets.len() - settings.end_packets;
  let late: PacketTiming = packets[late_start];
  for i in (late_start + 1)..packets.len() {
    if (packets[i].raw_delay_ms < late.raw_delay_ms) {
      late = packets[i];
    }
  }
  if (late.arrival_time_ms - early.arrival_time_ms < 1.0) {
    return None;
  }
  return Some((late.raw_delay_ms - early.raw_delay_ms) /
             (late.arrival_time_ms - early.arrival_time_ms) +
         settings.cap_uncertainty);
}

impl TrendlineEstimator {

const kMaxAdaptOffsetMs: f64 = 15.0;
const kOverUsingTimeThreshold: f64 = 10;
const kMinNumDeltas: isize = 60;
const kDeltaCounterMax: isize = 1000;

fn new(settings: TrendlineEstimatorSettings,
    network_state_predictor: NetworkStatePredictor) -> Self {
    tracing::info!("Using Trendline filter for delay change estimation with settings {:?} and {} network state predictor", settings, network_state_predictor ? "injected" : "no");
      Self {
        settings,
        smoothing_coef: Self::kDefaultTrendlineSmoothingCoeff,
        threshold_gain: Self::kDefaultTrendlineThresholdGain,
        num_of_deltas: 0,
        first_arrival_time_ms: -1,
        accumulated_delay: 0,
        smoothed_delay: 0,
        delay_hist: VecDeque::new(),
        k_up: 0.0087,
        k_down: 0.039,
        overusing_time_threshold: Self::kOverUsingTimeThreshold,
        threshold: 12.5,
        prev_modified_trend: NAN,
        last_update_ms: -1,
        prev_trend: 0.0,
        time_over_using: -1.0,
        overuse_counter: 0,
        hypothesis: BandwidthUsage::Normal,
        hypothesis_predicted: BandwidthUsage::Normal,
        network_state_predictor,
      }
}

fn UpdateTrendline(&mut self, recv_delta_ms: f64,
                                         send_delta_ms: f64,
                                         send_time_ms: i64,
                                         arrival_time_ms: i64,
                                         packet_size: usize) {
  let delta_ms: f64 = recv_delta_ms - send_delta_ms;
  self.num_of_deltas += 1;
  self.num_of_deltas = self.num_of_deltas.min(Self::kDeltaCounterMax);
  if (self.first_arrival_time_ms == -1) {
    self.first_arrival_time_ms = arrival_time_ms;
  }

  // Exponential backoff filter.
  self.accumulated_delay += delta_ms;
  self.smoothed_delay = self.smoothing_coef * self.smoothed_delay +
                    (1 - self.smoothing_coef) * self.accumulated_delay;

  // Maintain packet window
  self.delay_hist.push_back(
      (arrival_time_ms - self.first_arrival_time_ms) as f64,
      self.smoothed_delay, self.accumulated_delay);
  if self.settings.enable_sort {
    let i= self.delay_hist.len() - 1;
    while
         i > 0 &&
         self.delay_hist[i].arrival_time_ms < self.delay_hist[i - 1].arrival_time_ms {
        self.delay_hist.swap(i, i - 1);
         i -= 1;
        }
    }
  if (self.delay_hist.len() > self.settings.window_size) {
    self.delay_hist.pop_front();
  }

  // Simple linear regression.
  let trend: f64 = self.prev_trend;
  if (self.delay_hist.len() == self.settings.window_size) {
    // Update trend_ if it is possible to fit a line to the data. The delay
    // trend can be seen as an estimate of (send_rate - capacity)/capacity.
    // 0 < trend < 1   ->  the delay increases, queues are filling up
    //   trend == 0    ->  the delay does not change
    //   trend < 0     ->  the delay decreases, queues are being emptied
    trend = LinearFitSlope(&self.delay_hist).unwrap_or(trend);
    if (self.settings.enable_cap) {
      let cap= ComputeSlopeCap(self.delay_hist, self.settings);
      // We only use the cap to filter out overuse detections, not
      // to detect additional underuses.
      if let Some(cap) = cap {
        if (trend >= 0.0 && trend > cap) {
          trend = cap;
        }
      }
    }
  }
  self.Detect(trend, send_delta_ms, arrival_time_ms);
}

fn Update(&self /* TrendlineEstimator */,recv_delta_ms: f64,
                                send_delta_ms: f64,
                                send_time_ms: i64,
                                arrival_time_ms: i64,
                                packet_size: usize,
                                calculated_deltas: bool) {
}


fn Detect(&self /* TrendlineEstimator */,trend: f64, ts_delta: f64, now_ms: i64) {
  if (self.num_of_deltas < 2) {
    self.hypothesis = BandwidthUsage::Normal;
    return;
  }
  let modified_trend: f64 =
      std::cmp::min(self.num_of_deltas, Self::kMinNumDeltas) * trend * self.threshold_gain;
  self.prev_modified_trend = modified_trend;
  if (modified_trend > self.threshold) {
    if (self.time_over_using == -1.0) {
      // Initialize the timer. Assume that we've been
      // over-using half of the time since the previous
      // sample.
      self.time_over_using = ts_delta / 2;
    } else {
      // Increment timer
      self.time_over_using += ts_delta;
    }
    self.overuse_counter += 1;
    if (self.time_over_using > self.overusing_time_threshold && self.overuse_counter > 1) {
      if (trend >= self.prev_trend) {
        self.time_over_using = 0.0;
        self.overuse_counter = 0;
        self.hypothesis = BandwidthUsage::Overusing;
      }
    }
  } else if (modified_trend < -self.threshold) {
    self.time_over_using = -1.0;
    self.overuse_counter = 0;
    self.hypothesis = BandwidthUsage::Underusing;
  } else {
    self.time_over_using = -1.0;
    self.overuse_counter = 0;
    self.hypothesis = BandwidthUsage::Normal;
  }
  self.prev_trend = trend;
  self.UpdateThreshold(modified_trend, now_ms);
}

fn UpdateThreshold(&self /* TrendlineEstimator */,modified_trend: f64,
                                         now_ms: i64) {
  if (self.last_update_ms == -1) {
    self.last_update_ms = now_ms;
  }

  if (modified_trend.abs() > self.threshold + Self::kMaxAdaptOffsetMs) {
    // Avoid adapting the threshold to big latency spikes, caused e.g.,
    // by a sudden capacity drop.
    self.last_update_ms = now_ms;
    return;
  }

  let k: f64 = match modified_trend.abs() < self.threshold {
    true => self.k_down,
    false => self.k_up,
  };
  const kMaxTimeDeltaMs: i64 = 100;
  let time_delta_ms: i64 = std::cmp::min(now_ms - self.last_update_ms, kMaxTimeDeltaMs);
  self.threshold += k * (modified_trend.abs() - self.threshold) * time_delta_ms;
  self.threshold = self.threshold.clamp(6.0, 600.0);
  self.last_update_ms = now_ms;
}

}  // namespace webrtc
