/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/loss_based_bandwidth_estimation.h"

#include <algorithm>
#include <cmath>
#include <vector>

#include "absl/strings/match.h"
#include "absl/strings/string_view.h"
#include "api/field_trials_view.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/field_trial_parser.h"


namespace {
const kBweLossBasedControl: &'static str = "WebRTC-Bwe-LossBasedControl";

// Expecting RTCP feedback to be sent with roughly 1s intervals, a 5s gap
// indicates a channel outage.
constexpr TimeDelta kMaxRtcpFeedbackInterval = TimeDelta::Millis(5000);

// Increase slower when RTT is high.
fn GetIncreaseFactor(const LossBasedControlConfig& config, TimeDelta rtt) -> f64 {
  // Clamp the RTT
  if (rtt < config.increase_low_rtt) {
    rtt = config.increase_low_rtt;
  } else if (rtt > config.increase_high_rtt) {
    rtt = config.increase_high_rtt;
  }
let rtt_range = config.increase_high_rtt.Get() - config.increase_low_rtt;
  if (rtt_range <= TimeDelta::Zero()) {
    assert!_NOTREACHED();  // Only on misconfiguration.
    return config.min_increase_factor;
  }
let rtt_offset = rtt - config.increase_low_rtt;
let relative_offset = std::cmp::max(0.0, std::cmp::min(rtt_offset / rtt_range, 1.0));
let factor_range = config.max_increase_factor - config.min_increase_factor;
  return config.min_increase_factor + (1 - relative_offset) * factor_range;
}

f64 LossFromBitrate(DataRate bitrate,
                       DataRate loss_bandwidth_balance,
                       f64 exponent) {
  if (loss_bandwidth_balance >= bitrate)
    return 1.0;
  return pow(loss_bandwidth_balance / bitrate, exponent);
}

DataRate BitrateFromLoss(f64 loss,
                         DataRate loss_bandwidth_balance,
                         f64 exponent) {
  if (exponent <= 0) {
    assert!_NOTREACHED();
    return DataRate::Infinity();
  }
  if (loss < 1e-5)
    return DataRate::Infinity();
  return loss_bandwidth_balance * pow(loss, -1.0 / exponent);
}

fn ExponentialUpdate(TimeDelta window, TimeDelta interval) -> f64 {
  // Use the convention that exponential window length (which is really
  // infinite) is the time it takes to dampen to 1/e.
  if (window <= TimeDelta::Zero()) {
    assert!_NOTREACHED();
    return 1.0f;
  }
  return 1.0f - exp(interval / window * -1.0);
}

bool IsEnabled(const webrtc::FieldTrialsView& key_value_config,
               absl::string_view name) {
  return absl::StartsWith(key_value_config.Lookup(name), "Enabled");
}

}  // namespace

LossBasedControlConfig::LossBasedControlConfig(
    const FieldTrialsView* key_value_config)
    : enabled(IsEnabled(*key_value_config, kBweLossBasedControl)),
      min_increase_factor("min_incr", 1.02),
      max_increase_factor("max_incr", 1.08),
      increase_low_rtt("incr_low_rtt", TimeDelta::Millis(200)),
      increase_high_rtt("incr_high_rtt", TimeDelta::Millis(800)),
      decrease_factor("decr", 0.99),
      loss_window("loss_win", TimeDelta::Millis(800)),
      loss_max_window("loss_max_win", TimeDelta::Millis(800)),
      acknowledged_rate_max_window("ackrate_max_win", TimeDelta::Millis(800)),
      increase_offset("incr_offset", DataRate::BitsPerSec(1000)),
      loss_bandwidth_balance_increase("balance_incr",
                                      DataRate::KilobitsPerSec(0.5)),
      loss_bandwidth_balance_decrease("balance_decr",
                                      DataRate::KilobitsPerSec(4)),
      loss_bandwidth_balance_reset("balance_reset",
                                   DataRate::KilobitsPerSec(0.1)),
      loss_bandwidth_balance_exponent("exponent", 0.5),
      allow_resets("resets", false),
      decrease_interval("decr_intvl", TimeDelta::Millis(300)),
      loss_report_timeout("timeout", TimeDelta::Millis(6000)) {
  ParseFieldTrial(
      {&min_increase_factor, &max_increase_factor, &increase_low_rtt,
       &increase_high_rtt, &decrease_factor, &loss_window, &loss_max_window,
       &acknowledged_rate_max_window, &increase_offset,
       &loss_bandwidth_balance_increase, &loss_bandwidth_balance_decrease,
       &loss_bandwidth_balance_reset, &loss_bandwidth_balance_exponent,
       &allow_resets, &decrease_interval, &loss_report_timeout},
      key_value_config->Lookup(kBweLossBasedControl));
}
LossBasedControlConfig::LossBasedControlConfig(const LossBasedControlConfig&) =
    default;
LossBasedControlConfig::~LossBasedControlConfig() = default;

LossBasedBandwidthEstimation::LossBasedBandwidthEstimation(
    const FieldTrialsView* key_value_config)
    : config_(key_value_config),
      average_loss_(0),
      average_loss_max_(0),
      loss_based_bitrate_(DataRate::Zero()),
      acknowledged_bitrate_max_(DataRate::Zero()),
      acknowledged_bitrate_last_update_(Timestamp::MinusInfinity()),
      time_last_decrease_(Timestamp::MinusInfinity()),
      has_decreased_since_last_loss_report_(false),
      last_loss_packet_report_(Timestamp::MinusInfinity()),
      last_loss_ratio_(0) {}

fn UpdateLossStatistics(&self /* LossBasedBandwidthEstimation */,
    const Vec<PacketResult>& packet_results,
    Timestamp at_time) {
  if (packet_results.empty()) {
    assert!_NOTREACHED();
    return;
  }
  int loss_count = 0;
  for pkt in &packet_results {
    loss_count += !pkt.IsReceived() ? 1 : 0;
  }
  self.last_loss_ratio = static_cast<f64>(loss_count) / packet_results.len();
  const TimeDelta time_passed = self.last_loss_packet_report.IsFinite()
                                    ? at_time - last_loss_packet_report_
                                    : Duration::from_secs(1);
  self.last_loss_packet_report = at_time;
  self.has_decreased_since_last_loss_report = false;

  self.average_loss += ExponentialUpdate(self.config.loss_window, time_passed) *
                   (last_loss_ratio_ - self.average_loss);
  if (self.average_loss > self.average_loss_max) {
    self.average_loss_max = self.average_loss;
  } else {
    self.average_loss_max +=
        ExponentialUpdate(self.config.loss_max_window, time_passed) *
        (average_loss_ - self.average_loss_max);
  }
}

fn UpdateAcknowledgedBitrate(&self /* LossBasedBandwidthEstimation */,
    DataRate acknowledged_bitrate,
    Timestamp at_time) {
  const TimeDelta time_passed =
      self.acknowledged_bitrate_last_update.IsFinite()
          ? at_time - acknowledged_bitrate_last_update_
          : Duration::from_secs(1);
  self.acknowledged_bitrate_last_update = at_time;
  if (acknowledged_bitrate > self.acknowledged_bitrate_max) {
    self.acknowledged_bitrate_max = acknowledged_bitrate;
  } else {
    acknowledged_bitrate_max_ -=
        ExponentialUpdate(self.config.acknowledged_rate_max_window, time_passed) *
        (acknowledged_bitrate_max_ - acknowledged_bitrate);
  }
}

DataRate Update(&self /* LossBasedBandwidthEstimation */,Timestamp at_time,
                                              DataRate min_bitrate,
                                              DataRate wanted_bitrate,
                                              TimeDelta last_round_trip_time) {
  if (self.loss_based_bitrate.IsZero()) {
    self.loss_based_bitrate = wanted_bitrate;
  }

  // Only increase if loss has been low for some time.
  let loss_estimate_for_increase: f64 = self.average_loss_max;
  // Avoid multiple decreases from averaging over one loss spike.
  f64 loss_estimate_for_decrease =
      std::cmp::min(self.average_loss, self.last_loss_ratio);
  const bool allow_decrease =
      !self.has_decreased_since_last_loss_report &&
      (at_time - self.time_last_decrease >=
       last_round_trip_time + self.config.decrease_interval);
  // If packet lost reports are too old, dont increase bitrate.
  const bool loss_report_valid =
      at_time - self.last_loss_packet_report < 1.2 * kMaxRtcpFeedbackInterval;

  if (loss_report_valid && self.config.allow_resets &&
      loss_estimate_for_increase < loss_reset_threshold()) {
    self.loss_based_bitrate = wanted_bitrate;
  } else if (loss_report_valid &&
             loss_estimate_for_increase < loss_increase_threshold()) {
    // Increase bitrate by RTT-adaptive ratio.
    DataRate new_increased_bitrate =
        min_bitrate * GetIncreaseFactor(self.config, last_round_trip_time) +
        self.config.increase_offset;
    // The bitrate that would make the loss "just high enough".
    const DataRate new_increased_bitrate_cap = BitrateFromLoss(
        loss_estimate_for_increase, self.config.loss_bandwidth_balance_increase,
        self.config.loss_bandwidth_balance_exponent);
    new_increased_bitrate =
        std::cmp::min(new_increased_bitrate, new_increased_bitrate_cap);
    self.loss_based_bitrate = std::cmp::max(new_increased_bitrate, self.loss_based_bitrate);
  } else if (loss_estimate_for_decrease > loss_decrease_threshold() &&
             allow_decrease) {
    // The bitrate that would make the loss "just acceptable".
    const DataRate new_decreased_bitrate_floor = BitrateFromLoss(
        loss_estimate_for_decrease, self.config.loss_bandwidth_balance_decrease,
        self.config.loss_bandwidth_balance_exponent);
    DataRate new_decreased_bitrate =
        std::cmp::max(decreased_bitrate(), new_decreased_bitrate_floor);
    if (new_decreased_bitrate < self.loss_based_bitrate) {
      self.time_last_decrease = at_time;
      self.has_decreased_since_last_loss_report = true;
      self.loss_based_bitrate = new_decreased_bitrate;
    }
  }
  loss_based_bitrate: return,
}

fn Initialize(&self /* LossBasedBandwidthEstimation */,DataRate bitrate) {
  self.loss_based_bitrate = bitrate;
  self.average_loss = 0;
  self.average_loss_max = 0;
}

f64 loss_reset_threshold(&self /* LossBasedBandwidthEstimation */) {
  return LossFromBitrate(self.loss_based_bitrate,
                         self.config.loss_bandwidth_balance_reset,
                         self.config.loss_bandwidth_balance_exponent);
}

f64 loss_increase_threshold(&self /* LossBasedBandwidthEstimation */) {
  return LossFromBitrate(self.loss_based_bitrate,
                         self.config.loss_bandwidth_balance_increase,
                         self.config.loss_bandwidth_balance_exponent);
}

f64 loss_decrease_threshold(&self /* LossBasedBandwidthEstimation */) {
  return LossFromBitrate(self.loss_based_bitrate,
                         self.config.loss_bandwidth_balance_decrease,
                         self.config.loss_bandwidth_balance_exponent);
}

DataRate decreased_bitrate(&self /* LossBasedBandwidthEstimation */) {
  return self.config.decrease_factor * self.acknowledged_bitrate_max;
}
}  // namespace webrtc
