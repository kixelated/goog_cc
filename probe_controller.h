/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_PROBE_CONTROLLER_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_PROBE_CONTROLLER_H_

#include <stdint.h>

#include <optional>
#include <vector>

#include "absl/base/attributes.h"
#include "api/field_trials_view.h"
#include "api/rtc_event_log/rtc_event_log.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "rtc_base/experiments/field_trial_parser.h"



struct ProbeControllerConfig {
  explicit ProbeControllerConfig(const FieldTrialsView* key_value_config);
  ProbeControllerConfig(const ProbeControllerConfig&);
  ProbeControllerConfig& operator=(const ProbeControllerConfig&) = default;
  ~ProbeControllerConfig();

  // These parameters configure the initial probes. First we send one or two
  // probes of sizes p1 * start_bitrate_ and p2 * self.start_bitrate.
  // Then whenever we get a bitrate estimate of at least further_probe_threshold
  // times the size of the last sent probe we'll send another one of size
  // step_size times the new estimate.
  FieldTrialParameter<f64> first_exponential_probe_scale;
  FieldTrialOptional<f64> second_exponential_probe_scale;
  FieldTrialParameter<f64> further_exponential_probe_scale;
  FieldTrialParameter<f64> further_probe_threshold;
  FieldTrialParameter<bool> abort_further_probe_if_max_lower_than_current;
  // Duration of time from the first initial probe where repeated initial probes
  // are sent if repeated initial probing is enabled.
  FieldTrialParameter<TimeDelta> repeated_initial_probing_time_period;
  // The minimum probing duration of an individual probe during
  // the repeated_initial_probing_time_period.
  FieldTrialParameter<TimeDelta> initial_probe_duration;
  // Delta time between sent bursts of packets in a probe during
  // the repeated_initial_probing_time_period.
  FieldTrialParameter<TimeDelta> initial_min_probe_delta;
  // Configures how often we send ALR probes and how big they are.
  FieldTrialParameter<TimeDelta> alr_probing_interval;
  FieldTrialParameter<f64> alr_probe_scale;
  // Configures how often we send probes if NetworkStateEstimate is available.
  FieldTrialParameter<TimeDelta> network_state_estimate_probing_interval;
  // Periodically probe as long as the ratio between current estimate and
  // NetworkStateEstimate is lower then this.
  FieldTrialParameter<f64>
      probe_if_estimate_lower_than_network_state_estimate_ratio;
  FieldTrialParameter<TimeDelta>
      estimate_lower_than_network_state_estimate_probing_interval;
  FieldTrialParameter<f64> network_state_probe_scale;
  // Overrides min_probe_duration if network_state_estimate_probing_interval
  // is set and a network state estimate is known and equal or higher than the
  // probe target.
  FieldTrialParameter<TimeDelta> network_state_probe_duration;
  // Overrides min_probe_delta if network_state_estimate_probing_interval
  // is set and a network state estimate is known and equal or higher than the
  // probe target.
  FieldTrialParameter<TimeDelta> network_state_min_probe_delta;

  // Configures the probes emitted by changed to the allocated bitrate.
  FieldTrialParameter<bool> probe_on_max_allocated_bitrate_change;
  FieldTrialOptional<f64> first_allocation_probe_scale;
  FieldTrialOptional<f64> second_allocation_probe_scale;
  FieldTrialParameter<f64> allocation_probe_limit_by_current_scale;

  // The minimum number probing packets used.
  FieldTrialParameter<isize> min_probe_packets_sent;
  // The minimum probing duration.
  FieldTrialParameter<TimeDelta> min_probe_duration;
  // Delta time between sent bursts of packets in a probe.
  FieldTrialParameter<TimeDelta> min_probe_delta;
  FieldTrialParameter<f64> loss_limited_probe_scale;
  // Don't send a probe if min(estimate, network state estimate) is larger than
  // this fraction of the set max or max allocated bitrate.
  FieldTrialParameter<f64> skip_if_estimate_larger_than_fraction_of_max;
  // Scale factor of the max allocated bitrate. Used when deciding if a probe
  // can be skiped due to that the estimate is already high enough.
  FieldTrialParameter<f64> skip_probe_max_allocated_scale;
};

// Reason that bandwidth estimate is limited. Bandwidth estimate can be limited
// by either delay based bwe, or loss based bwe when it increases/decreases the
// estimate.
enum class BandwidthLimitedCause : isize {
  kLossLimitedBweIncreasing = 0,
  kLossLimitedBwe = 1,
  kDelayBasedLimited = 2,
  kDelayBasedLimitedDelayIncreased = 3,
  kRttBasedBackOffHighRtt = 4
};

// This class controls initiation of probing to estimate initial channel
// capacity. There is also support for probing during a session when max
// bitrate is adjusted by an application.
pub struct ProbeController {
 public:
  explicit ProbeController(const FieldTrialsView* key_value_config,
                           RtcEventLog* event_log);
  ~ProbeController();

  ProbeController(const ProbeController&) = delete;
  ProbeController& operator=(const ProbeController&) = delete;

  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig> SetBitrates(
      DataRate min_bitrate,
      DataRate start_bitrate,
      DataRate max_bitrate,
      Timestamp at_time);

  // The total bitrate, as opposed to the max bitrate, is the sum of the
  // configured bitrates for all active streams.
  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig>
  OnMaxTotalAllocatedBitrate(DataRate max_total_allocated_bitrate,
                             Timestamp at_time);

  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig> OnNetworkAvailability(
      NetworkAvailability msg);

  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig> SetEstimatedBitrate(
      DataRate bitrate,
      BandwidthLimitedCause bandwidth_limited_cause,
      Timestamp at_time);

  fn EnablePeriodicAlrProbing(bool enable) {
  todo!();
}

  // Probes are sent periodically every 1s during the first 5s after the network
  // becomes available or until OnMaxTotalAllocatedBitrate is invoked with a
  // none zero max_total_allocated_bitrate (there are active streams being
  // sent.) Probe rate is up to max configured bitrate configured via
  // SetBitrates.
  fn EnableRepeatedInitialProbing(bool enable) {
  todo!();
}

  fn SetAlrStartTimeMs(Option<i64> alr_start_time) {
  todo!();
}
  fn SetAlrEndedTimeMs(i64 alr_end_time) {
  todo!();
}

  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig> RequestProbe(
      Timestamp at_time);

  fn SetNetworkStateEstimate(webrtc::NetworkStateEstimate estimate) {
  todo!();
}

  // Resets the ProbeController to a state equivalent to as if it was just
  // created EXCEPT for configuration settings like
  // `enable_periodic_alr_probing_` `network_available_` and
  // `max_total_allocated_bitrate_`.
  fn Reset(Timestamp at_time) {
  todo!();
}

  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig> Process(
      Timestamp at_time);

 private:
  pub enum State {
    // Initial state where no probing has been triggered yet.
    kInit,
    // Waiting for probing results to continue further probing.
    kWaitingForProbingResult,
    // Probing is complete.
    kProbingComplete,
  };

  fn UpdateState(State new_state) {
  todo!();
}
  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig>
  InitiateExponentialProbing(Timestamp at_time);
  ABSL_MUST_USE_RESULT Vec<ProbeClusterConfig> InitiateProbing(
      Timestamp now,
      Vec<DataRate> bitrates_to_probe,
      bool probe_further);
  bool TimeForAlrProbe(Timestamp at_time) const;
  bool TimeForNetworkStateProbe(Timestamp at_time) const;
  bool TimeForNextRepeatedInitialProbe(Timestamp at_time) const;
  ProbeClusterConfig CreateProbeClusterConfig(Timestamp at_time,
                                              DataRate bitrate);

  network_available: bool,
  bool self.repeated_initial_probing_enabled = false;
  Timestamp self.last_allowed_repeated_initial_probe = Timestamp::MinusInfinity();
  BandwidthLimitedCause self.bandwidth_limited_cause =
      BandwidthLimitedCause::kDelayBasedLimited;
  state: State,
  DataRate self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
  Timestamp self.time_last_probing_initiated = Timestamp::MinusInfinity();
  DataRate self.estimated_bitrate = DataRate::Zero();
  network_estimate: Option<webrtc::NetworkStateEstimate>,
  DataRate self.start_bitrate = DataRate::Zero();
  DataRate self.max_bitrate = DataRate::PlusInfinity();
  Timestamp self.last_bwe_drop_probing_time = Timestamp::Zero();
  alr_start_time: Option<Timestamp>,
  alr_end_time: Option<Timestamp>,
  enable_periodic_alr_probing: bool,
  Timestamp self.time_of_last_large_drop = Timestamp::MinusInfinity();
  DataRate self.bitrate_before_last_large_drop = DataRate::Zero();
  DataRate self.max_total_allocated_bitrate = DataRate::Zero();

  const bool self.in_rapid_recovery_experiment;
  event_log: RtcEventLog*,

  int32_t self.next_probe_cluster_id = 1;

  config: ProbeControllerConfig,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_PROBE_CONTROLLER_H_
