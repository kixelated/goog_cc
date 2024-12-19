/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_LOSS_BASED_BANDWIDTH_ESTIMATION_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_LOSS_BASED_BANDWIDTH_ESTIMATION_H_

#include <vector>

#include "api/field_trials_view.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "rtc_base/experiments/field_trial_parser.h"



struct LossBasedControlConfig {
  explicit LossBasedControlConfig(const FieldTrialsView* key_value_config);
  LossBasedControlConfig(const LossBasedControlConfig&);
  LossBasedControlConfig& operator=(const LossBasedControlConfig&) = default;
  ~LossBasedControlConfig();
  bool enabled;
  FieldTrialParameter<f64> min_increase_factor;
  FieldTrialParameter<f64> max_increase_factor;
  FieldTrialParameter<TimeDelta> increase_low_rtt;
  FieldTrialParameter<TimeDelta> increase_high_rtt;
  FieldTrialParameter<f64> decrease_factor;
  FieldTrialParameter<TimeDelta> loss_window;
  FieldTrialParameter<TimeDelta> loss_max_window;
  FieldTrialParameter<TimeDelta> acknowledged_rate_max_window;
  FieldTrialParameter<DataRate> increase_offset;
  FieldTrialParameter<DataRate> loss_bandwidth_balance_increase;
  FieldTrialParameter<DataRate> loss_bandwidth_balance_decrease;
  FieldTrialParameter<DataRate> loss_bandwidth_balance_reset;
  FieldTrialParameter<f64> loss_bandwidth_balance_exponent;
  FieldTrialParameter<bool> allow_resets;
  FieldTrialParameter<TimeDelta> decrease_interval;
  FieldTrialParameter<TimeDelta> loss_report_timeout;
};

// Estimates an upper BWE limit based on loss.
// It requires knowledge about lost packets and acknowledged bitrate.
// Ie, this class require transport feedback.
pub struct LossBasedBandwidthEstimation {
 public:
  explicit LossBasedBandwidthEstimation(
      const FieldTrialsView* key_value_config);
  // Returns the new estimate.
  DataRate Update(Timestamp at_time,
                  DataRate min_bitrate,
                  DataRate wanted_bitrate,
                  TimeDelta last_round_trip_time);
  void UpdateAcknowledgedBitrate(DataRate acknowledged_bitrate,
                                 Timestamp at_time);
  fn Initialize(DataRate bitrate) {
  todo!();
}
  bool Enabled() { return self.config.enabled; }
  // Returns true if LossBasedBandwidthEstimation is enabled and have
  // received loss statistics. Ie, this class require transport feedback.
  bool InUse() {
    return Enabled() && self.last_loss_packet_report.IsFinite();
  }
  void UpdateLossStatistics(const Vec<PacketResult>& packet_results,
                            Timestamp at_time);
  DataRate GetEstimate() { return self.loss_based_bitrate; }

 private:
  friend class GoogCcStatePrinter;
  fn Reset(DataRate bitrate) {
  todo!();
}
  f64 loss_increase_threshold() const;
  f64 loss_decrease_threshold() const;
  f64 loss_reset_threshold() const;

  DataRate decreased_bitrate() const;

  const LossBasedControlConfig self.config;
  average_loss: f64,
  average_loss_max: f64,
  loss_based_bitrate: DataRate,
  acknowledged_bitrate_max: DataRate,
  acknowledged_bitrate_last_update: Timestamp,
  time_last_decrease: Timestamp,
  has_decreased_since_last_loss_report: bool,
  last_loss_packet_report: Timestamp,
  last_loss_ratio: f64,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_LOSS_BASED_BANDWIDTH_ESTIMATION_H_
