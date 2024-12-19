/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/probe_controller.h"

#include <algorithm>
#include <cstdint>
#include <initializer_list>
#include <memory>
#include <optional>
#include <vector>

#include "absl/strings/match.h"
#include "api/field_trials_view.h"
#include "api/rtc_event_log/rtc_event_log.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/events/rtc_event_probe_cluster_created.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/field_trial_parser.h"
#include "rtc_base/logging.h"
#include "system_wrappers/include/metrics.h"



namespace {
// Maximum waiting time from the time of initiating probing to getting
// the measured results back.
const MaxWaitingTimeForProbingResult: TimeDelta = Duration::from_secs(1);

// Default probing bitrate limit. Applied only when the application didn't
// specify max bitrate.
const DefaultMaxProbingBitrate: DataRate = DataRate::KilobitsPerSec(5000);

// If the bitrate drops to a factor `kBitrateDropThreshold` or lower
// and we recover within `kBitrateDropTimeoutMs`, then we'll send
// a probe at a fraction `kProbeFractionAfterDrop` of the original bitrate.
const kBitrateDropThreshold: f64 = 0.66;
const BitrateDropTimeout: TimeDelta = Duration::from_secs(5);
const kProbeFractionAfterDrop: f64 = 0.85;

// Timeout for probing after leaving ALR. If the bitrate drops significantly,
// (as determined by the delay based estimator) and we leave ALR, then we will
// send a probe if we recover within `kLeftAlrTimeoutMs` ms.
const AlrEndedTimeout: TimeDelta = Duration::from_secs(3);

// The expected uncertainty of probe result (as a fraction of the target probe
// This is a limit on how often probing can be done when there is a BW
// drop detected in ALR.
const MinTimeBetweenAlrProbes: TimeDelta = Duration::from_secs(5);

// bitrate). Used to avoid probing if the probe bitrate is close to our current
// estimate.
const kProbeUncertainty: f64 = 0.05;

// Use probing to recover faster after large bitrate estimate drops.
constexpr char kBweRapidRecoveryExperiment[] =
    "WebRTC-BweRapidRecoveryExperiment";

fn MaybeLogProbeClusterCreated(RtcEventLog* event_log,
                                 const ProbeClusterConfig& probe) {
  assert!(event_log);
  if (!event_log) {
    return;
  }

  let min_data_size: DataSize = probe.target_data_rate * probe.target_duration;
  event_log->Log(std::make_unique<RtcEventProbeClusterCreated>(
      probe.id, probe.target_data_rate.bps(), probe.target_probe_count,
      min_data_size.bytes()));
}

}  // namespace

ProbeControllerConfig::ProbeControllerConfig(
    const FieldTrialsView* key_value_config)
    : first_exponential_probe_scale("p1", 3.0),
      second_exponential_probe_scale("p2", 6.0),
      further_exponential_probe_scale("step_size", 2),
      further_probe_threshold("further_probe_threshold", 0.7),
      abort_further_probe_if_max_lower_than_current("abort_further", false),
      repeated_initial_probing_time_period("initial_probing",
                                           Duration::from_secs(5)),
      initial_probe_duration("initial_probe_duration", TimeDelta::Millis(100)),
      initial_min_probe_delta("initial_min_probe_delta", TimeDelta::Millis(20)),
      alr_probing_interval("alr_interval", Duration::from_secs(5)),
      alr_probe_scale("alr_scale", 2),
      network_state_estimate_probing_interval("network_state_interval",
                                              TimeDelta::PlusInfinity()),
      probe_if_estimate_lower_than_network_state_estimate_ratio(
          "est_lower_than_network_ratio",
          0),
      estimate_lower_than_network_state_estimate_probing_interval(
          "est_lower_than_network_interval",
          Duration::from_secs(3)),
      network_state_probe_scale("network_state_scale", 1.0),
      network_state_probe_duration("network_state_probe_duration",
                                   TimeDelta::Millis(15)),
      network_state_min_probe_delta("network_state_min_probe_delta",
                                    TimeDelta::Millis(20)),
      probe_on_max_allocated_bitrate_change("probe_max_allocation", true),
      first_allocation_probe_scale("alloc_p1", 1),
      second_allocation_probe_scale("alloc_p2", 2),
      allocation_probe_limit_by_current_scale("alloc_current_bwe_limit", 2),
      min_probe_packets_sent("min_probe_packets_sent", 5),
      min_probe_duration("min_probe_duration", TimeDelta::Millis(15)),
      min_probe_delta("min_probe_delta", TimeDelta::Millis(2)),
      loss_limited_probe_scale("loss_limited_scale", 1.5),
      skip_if_estimate_larger_than_fraction_of_max(
          "skip_if_est_larger_than_fraction_of_max",
          0.0),
      skip_probe_max_allocated_scale("skip_max_allocated_scale", 1.0) {
  ParseFieldTrial({&first_exponential_probe_scale,
                   &second_exponential_probe_scale,
                   &further_exponential_probe_scale,
                   &further_probe_threshold,
                   &abort_further_probe_if_max_lower_than_current,
                   &repeated_initial_probing_time_period,
                   &initial_probe_duration,
                   &alr_probing_interval,
                   &alr_probe_scale,
                   &probe_on_max_allocated_bitrate_change,
                   &first_allocation_probe_scale,
                   &second_allocation_probe_scale,
                   &allocation_probe_limit_by_current_scale,
                   &min_probe_duration,
                   &min_probe_delta,
                   &initial_min_probe_delta,
                   &network_state_estimate_probing_interval,
                   &network_state_min_probe_delta,
                   &probe_if_estimate_lower_than_network_state_estimate_ratio,
                   &estimate_lower_than_network_state_estimate_probing_interval,
                   &network_state_probe_scale,
                   &network_state_probe_duration,
                   &min_probe_packets_sent,
                   &loss_limited_probe_scale,
                   &skip_if_estimate_larger_than_fraction_of_max,
                   &skip_probe_max_allocated_scale},
                  key_value_config->Lookup("WebRTC-Bwe-ProbingConfiguration"));

  // Specialized keys overriding subsets of WebRTC-Bwe-ProbingConfiguration
  ParseFieldTrial(
      {&first_exponential_probe_scale, &second_exponential_probe_scale},
      key_value_config->Lookup("WebRTC-Bwe-InitialProbing"));
  ParseFieldTrial({&further_exponential_probe_scale, &further_probe_threshold},
                  key_value_config->Lookup("WebRTC-Bwe-ExponentialProbing"));
  ParseFieldTrial(
      {&alr_probing_interval, &alr_probe_scale, &loss_limited_probe_scale},
      key_value_config->Lookup("WebRTC-Bwe-AlrProbing"));
  ParseFieldTrial(
      {&first_allocation_probe_scale, &second_allocation_probe_scale,
       &allocation_probe_limit_by_current_scale},
      key_value_config->Lookup("WebRTC-Bwe-AllocationProbing"));
  ParseFieldTrial({&min_probe_packets_sent, &min_probe_duration},
                  key_value_config->Lookup("WebRTC-Bwe-ProbingBehavior"));
}

ProbeControllerConfig::ProbeControllerConfig(const ProbeControllerConfig&) =
    default;
ProbeControllerConfig::~ProbeControllerConfig() = default;

ProbeController::ProbeController(const FieldTrialsView* key_value_config,
                                 RtcEventLog* event_log)
    : network_available_(false),
      enable_periodic_alr_probing_(false),
      in_rapid_recovery_experiment_(absl::StartsWith(
          key_value_config->Lookup(kBweRapidRecoveryExperiment),
          "Enabled")),
      event_log_(event_log),
      config_(ProbeControllerConfig(key_value_config)) {
  Reset(Timestamp::Zero());
}

ProbeController::~ProbeController() {}

Vec<ProbeClusterConfig> SetBitrates(&self /* ProbeController */,
    DataRate min_bitrate,
    DataRate start_bitrate,
    DataRate max_bitrate,
    Timestamp at_time) {
  if (start_bitrate > DataRate::Zero()) {
    self.start_bitrate = start_bitrate;
    self.estimated_bitrate = start_bitrate;
  } else if (self.start_bitrate.IsZero()) {
    self.start_bitrate = min_bitrate;
  }

  // The reason we use the variable `old_max_bitrate_pbs` is because we
  // need to set `max_bitrate_` before we call InitiateProbing.
  let old_max_bitrate: DataRate = self.max_bitrate;
  self.max_bitrate =
      max_bitrate.IsFinite() ? max_bitrate : kDefaultMaxProbingBitrate;

  switch (self.state) {
    case State::kInit:
      if (self.network_available)
        return InitiateExponentialProbing(at_time);
      break;

    case State::kWaitingForProbingResult:
      break;

    case State::kProbingComplete:
      // If the new max bitrate is higher than both the old max bitrate and the
      // estimate then initiate probing.
      if (!self.estimated_bitrate.IsZero() && old_max_bitrate < self.max_bitrate &&
          self.estimated_bitrate < self.max_bitrate) {
        return InitiateProbing(at_time, {max_bitrate_}, false);
      }
      break;
  }
  return Vec<ProbeClusterConfig>();
}

Vec<ProbeClusterConfig> OnMaxTotalAllocatedBitrate(&self /* ProbeController */,
    DataRate max_total_allocated_bitrate,
    Timestamp at_time) {
  const bool in_alr = self.alr_start_time.is_some();
  const bool allow_allocation_probe = in_alr;
  if (self.config.probe_on_max_allocated_bitrate_change &&
      self.state == State::kProbingComplete &&
      max_total_allocated_bitrate != self.max_total_allocated_bitrate &&
      self.estimated_bitrate < self.max_bitrate &&
      self.estimated_bitrate < max_total_allocated_bitrate &&
      allow_allocation_probe) {
    self.max_total_allocated_bitrate = max_total_allocated_bitrate;

    if (!self.config.first_allocation_probe_scale)
      return Vec<ProbeClusterConfig>();

    let first_probe_rate: DataRate = max_total_allocated_bitrate *
                                self.config.first_allocation_probe_scale.Value();
    const DataRate current_bwe_limit =
        self.config.allocation_probe_limit_by_current_scale.Get() *
        self.estimated_bitrate;
    let limited_by_current_bwe: bool = current_bwe_limit < first_probe_rate;
    if (limited_by_current_bwe) {
      first_probe_rate = current_bwe_limit;
    }

    Vec<DataRate> probes = {first_probe_rate};
    if (!limited_by_current_bwe && self.config.second_allocation_probe_scale) {
      let second_probe_rate: DataRate =
          max_total_allocated_bitrate *
          self.config.second_allocation_probe_scale.Value();
      limited_by_current_bwe = current_bwe_limit < second_probe_rate;
      if (limited_by_current_bwe) {
        second_probe_rate = current_bwe_limit;
      }
      if (second_probe_rate > first_probe_rate)
        probes.push_back(second_probe_rate);
    }
    let allow_further_probing: bool = limited_by_current_bwe;

    return InitiateProbing(at_time, probes, allow_further_probing);
  }
  if (!max_total_allocated_bitrate.IsZero()) {
    self.last_allowed_repeated_initial_probe = at_time;
  }

  self.max_total_allocated_bitrate = max_total_allocated_bitrate;
  return Vec<ProbeClusterConfig>();
}

Vec<ProbeClusterConfig> OnNetworkAvailability(&self /* ProbeController */,
    NetworkAvailability msg) {
  self.network_available = msg.network_available;

  if (!self.network_available && self.state == State::kWaitingForProbingResult) {
    self.state = State::kProbingComplete;
    self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
  }

  if (self.network_available && self.state == State::kInit && !self.start_bitrate.IsZero())
    return InitiateExponentialProbing(msg.at_time);
  return Vec<ProbeClusterConfig>();
}

fn UpdateState(&self /* ProbeController */,State new_state) {
  switch (new_state) {
    case State::kInit:
      self.state = State::kInit;
      break;
    case State::kWaitingForProbingResult:
      self.state = State::kWaitingForProbingResult;
      break;
    case State::kProbingComplete:
      self.state = State::kProbingComplete;
      self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
      break;
  }
}

Vec<ProbeClusterConfig> InitiateExponentialProbing(&self /* ProbeController */,
    Timestamp at_time) {
  assert!(self.network_available);
  assert!(self.state == State::kInit);
  assert!_GT(self.start_bitrate, DataRate::Zero());

  // When probing at 1.8 Mbps ( 6x 300), this represents a threshold of
  // 1.2 Mbps to continue probing.
  Vec<DataRate> probes = {self.config.first_exponential_probe_scale *
                                  start_bitrate_};
  if (self.config.second_exponential_probe_scale &&
      self.config.second_exponential_probe_scale.GetOptional().value() > 0) {
    probes.push_back(self.config.second_exponential_probe_scale.Value() *
                     self.start_bitrate);
  }
  if (self.repeated_initial_probing_enabled &&
      self.max_total_allocated_bitrate.IsZero()) {
    self.last_allowed_repeated_initial_probe =
        at_time + self.config.repeated_initial_probing_time_period;
    RTC_LOG(LS_INFO) << "Repeated initial probing enabled, last allowed probe: "
                     << last_allowed_repeated_initial_probe_
                     << " now: " << at_time;
  }

  return InitiateProbing(at_time, probes, true);
}

Vec<ProbeClusterConfig> SetEstimatedBitrate(&self /* ProbeController */,
    DataRate bitrate,
    BandwidthLimitedCause bandwidth_limited_cause,
    Timestamp at_time) {
  self.bandwidth_limited_cause = bandwidth_limited_cause;
  if (bitrate < kBitrateDropThreshold * self.estimated_bitrate) {
    self.time_of_last_large_drop = at_time;
    self.bitrate_before_last_large_drop = self.estimated_bitrate;
  }
  self.estimated_bitrate = bitrate;

  if (self.state == State::kWaitingForProbingResult) {
    // Continue probing if probing results indicate channel has greater
    // capacity unless we already reached the needed bitrate.
    if (self.config.abort_further_probe_if_max_lower_than_current &&
        (bitrate > self.max_bitrate ||
         (!self.max_total_allocated_bitrate.IsZero() &&
          bitrate > 2 * self.max_total_allocated_bitrate))) {
      // No need to continue probing.
      self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
    }
    let network_state_estimate_probe_further_limit: DataRate =
        self.config.network_state_estimate_probing_interval->IsFinite() &&
                self.network_estimate &&
                self.network_estimate.link_capacity_upper.IsFinite()
            ? self.network_estimate.link_capacity_upper *
                  self.config.further_probe_threshold
            : DataRate::PlusInfinity();
    RTC_LOG(LS_INFO) << "Measured bitrate: " << bitrate
                     << " Minimum to probe further: "
                     << self.min_bitrate_to_probe_further << " upper limit: "
                     << network_state_estimate_probe_further_limit;

    if (bitrate > self.min_bitrate_to_probe_further &&
        bitrate <= network_state_estimate_probe_further_limit) {
      return InitiateProbing(
          at_time, {self.config.further_exponential_probe_scale * bitrate}, true);
    }
  }
  return {};
}

fn EnablePeriodicAlrProbing(&self /* ProbeController */,bool enable) {
  self.enable_periodic_alr_probing = enable;
}

fn EnableRepeatedInitialProbing(&self /* ProbeController */,bool enable) {
  self.repeated_initial_probing_enabled = enable;
}

fn SetAlrStartTimeMs(&self /* ProbeController */,
    Option<i64> alr_start_time_ms) {
  if (alr_start_time_ms) {
    self.alr_start_time = Timestamp::Millis(*alr_start_time_ms);
  } else {
    self.alr_start_time = None;
  }
}
fn SetAlrEndedTimeMs(&self /* ProbeController */,i64 alr_end_time_ms) {
  self.alr_end_time.emplace(Timestamp::Millis(alr_end_time_ms));
}

Vec<ProbeClusterConfig> RequestProbe(&self /* ProbeController */,
    Timestamp at_time) {
  // Called once we have returned to normal state after a large drop in
  // estimated bandwidth. The current response is to initiate a single probe
  // session (if not already probing) at the previous bitrate.
  //
  // If the probe session fails, the assumption is that this drop was a
  // real one from a competing flow or a network change.
  let in_alr: bool = self.alr_start_time.is_some();
  let alr_ended_recently: bool =
      (self.alr_end_time.is_some() &&
       at_time - self.alr_end_time.value() < kAlrEndedTimeout);
  if (in_alr || alr_ended_recently || self.in_rapid_recovery_experiment) {
    if (self.state == State::kProbingComplete) {
      let suggested_probe: DataRate =
          kProbeFractionAfterDrop * self.bitrate_before_last_large_drop;
      let min_expected_probe_result: DataRate =
          (1 - kProbeUncertainty) * suggested_probe;
      let time_since_drop: TimeDelta = at_time - self.time_of_last_large_drop;
      let time_since_probe: TimeDelta = at_time - self.last_bwe_drop_probing_time;
      if (min_expected_probe_result > self.estimated_bitrate &&
          time_since_drop < kBitrateDropTimeout &&
          time_since_probe > kMinTimeBetweenAlrProbes) {
        RTC_LOG(LS_INFO) << "Detected big bandwidth drop, start probing.";
        // Track how often we probe in response to bandwidth drop in ALR.
        RTC_HISTOGRAM_COUNTS_10000(
            "WebRTC.BWE.BweDropProbingIntervalInS",
            (at_time - self.last_bwe_drop_probing_time).seconds());
        self.last_bwe_drop_probing_time = at_time;
        return InitiateProbing(at_time, {suggested_probe}, false);
      }
    }
  }
  return Vec<ProbeClusterConfig>();
}

fn SetNetworkStateEstimate(&self /* ProbeController */,
    webrtc::NetworkStateEstimate estimate) {
  self.network_estimate = estimate;
}

fn Reset(&self /* ProbeController */,Timestamp at_time) {
  self.bandwidth_limited_cause = BandwidthLimitedCause::kDelayBasedLimited;
  self.state = State::kInit;
  self.min_bitrate_to_probe_further = DataRate::PlusInfinity();
  self.time_last_probing_initiated = Timestamp::Zero();
  self.estimated_bitrate = DataRate::Zero();
  self.network_estimate = None;
  self.start_bitrate = DataRate::Zero();
  self.max_bitrate = kDefaultMaxProbingBitrate;
  let now: Timestamp = at_time;
  self.last_bwe_drop_probing_time = now;
  self.alr_end_time.reset();
  self.time_of_last_large_drop = now;
  self.bitrate_before_last_large_drop = DataRate::Zero();
}

bool TimeForAlrProbe(&self /* ProbeController */,Timestamp at_time) {
  if (self.enable_periodic_alr_probing && self.alr_start_time) {
    let next_probe_time: Timestamp =
        std::cmp::max(*self.alr_start_time, self.time_last_probing_initiated) +
        self.config.alr_probing_interval;
    return at_time >= next_probe_time;
  }
  return false;
}

bool TimeForNetworkStateProbe(&self /* ProbeController */,Timestamp at_time) {
  if (!self.network_estimate ||
      self.network_estimate.link_capacity_upper.IsInfinite()) {
    return false;
  }

  let probe_due_to_low_estimate: bool =
      self.bandwidth_limited_cause == BandwidthLimitedCause::kDelayBasedLimited &&
      self.estimated_bitrate <
          self.config.probe_if_estimate_lower_than_network_state_estimate_ratio *
              self.network_estimate.link_capacity_upper;
  if (probe_due_to_low_estimate &&
      self.config.estimate_lower_than_network_state_estimate_probing_interval
          ->IsFinite()) {
    let next_probe_time: Timestamp =
        self.time_last_probing_initiated +
        self.config.estimate_lower_than_network_state_estimate_probing_interval;
    return at_time >= next_probe_time;
  }

  let periodic_probe: bool =
      self.estimated_bitrate < self.network_estimate.link_capacity_upper;
  if (periodic_probe &&
      self.config.network_state_estimate_probing_interval->IsFinite()) {
    let next_probe_time: Timestamp = self.time_last_probing_initiated +
                                self.config.network_state_estimate_probing_interval;
    return at_time >= next_probe_time;
  }

  return false;
}

bool TimeForNextRepeatedInitialProbe(&self /* ProbeController */,Timestamp at_time) {
  if (state_ != State::kWaitingForProbingResult &&
      self.last_allowed_repeated_initial_probe > at_time) {
    let next_probe_time: Timestamp =
        self.time_last_probing_initiated + kMaxWaitingTimeForProbingResult;
    if (at_time >= next_probe_time) {
      return true;
    }
  }
  return false;
}

Vec<ProbeClusterConfig> Process(&self /* ProbeController */,Timestamp at_time) {
  if (at_time - self.time_last_probing_initiated >
      kMaxWaitingTimeForProbingResult) {
    if (self.state == State::kWaitingForProbingResult) {
      RTC_LOG(LS_INFO) << "kWaitingForProbingResult: timeout";
      UpdateState(State::kProbingComplete);
    }
  }
  if (self.estimated_bitrate.IsZero() || state_ != State::kProbingComplete) {
    return {};
  }
  if (TimeForNextRepeatedInitialProbe(at_time)) {
    return InitiateProbing(
        at_time, {self.estimated_bitrate * self.config.first_exponential_probe_scale},
        true);
  }
  if (TimeForAlrProbe(at_time) || TimeForNetworkStateProbe(at_time)) {
    return InitiateProbing(
        at_time, {self.estimated_bitrate * self.config.alr_probe_scale}, true);
  }
  return Vec<ProbeClusterConfig>();
}

ProbeClusterConfig CreateProbeClusterConfig(&self /* ProbeController */,Timestamp at_time,
                                                             DataRate bitrate) {
  ProbeClusterConfig config;
  config.at_time = at_time;
  config.target_data_rate = bitrate;
  if (self.network_estimate &&
      self.config.network_state_estimate_probing_interval->IsFinite() &&
      self.network_estimate.link_capacity_upper.IsFinite() &&
      self.network_estimate.link_capacity_upper >= bitrate) {
    config.target_duration = self.config.network_state_probe_duration;
    config.min_probe_delta = self.config.network_state_min_probe_delta;
  } else if (at_time < self.last_allowed_repeated_initial_probe) {
    config.target_duration = self.config.initial_probe_duration;
    config.min_probe_delta = self.config.initial_min_probe_delta;
  } else {
    config.target_duration = self.config.min_probe_duration;
    config.min_probe_delta = self.config.min_probe_delta;
  }
  config.target_probe_count = self.config.min_probe_packets_sent;
  config.id = self.next_probe_cluster_id;
  self.next_probe_cluster_id += 1;
  MaybeLogProbeClusterCreated(self.event_log, config);
  return config;
}

Vec<ProbeClusterConfig> InitiateProbing(&self /* ProbeController */,
    Timestamp now,
    Vec<DataRate> bitrates_to_probe,
    bool probe_further) {
  if (self.config.skip_if_estimate_larger_than_fraction_of_max > 0) {
    let network_estimate: DataRate = network_estimate_
                                    ? self.network_estimate.link_capacity_upper
                                    : DataRate::PlusInfinity();
    let max_probe_rate: DataRate =
        self.max_total_allocated_bitrate.IsZero()
            ? max_bitrate_
            : std::cmp::min(self.config.skip_probe_max_allocated_scale *
                           self.max_total_allocated_bitrate,
                       self.max_bitrate);
    if (std::cmp::min(network_estimate, self.estimated_bitrate) >
        self.config.skip_if_estimate_larger_than_fraction_of_max * max_probe_rate) {
      UpdateState(State::kProbingComplete);
      return {};
    }
  }

  let max_probe_bitrate: DataRate = self.max_bitrate;
  if (self.max_total_allocated_bitrate > DataRate::Zero()) {
    // If a max allocated bitrate has been configured, allow probing up to 2x
    // that rate. This allows some overhead to account for bursty streams,
    // which otherwise would have to ramp up when the overshoot is already in
    // progress.
    // It also avoids minor quality reduction caused by probes often being
    // received at slightly less than the target probe bitrate.
    max_probe_bitrate =
        std::cmp::min(max_probe_bitrate, self.max_total_allocated_bitrate * 2);
  }

  switch (self.bandwidth_limited_cause) {
    case BandwidthLimitedCause::kRttBasedBackOffHighRtt:
    case BandwidthLimitedCause::kDelayBasedLimitedDelayIncreased:
    case BandwidthLimitedCause::kLossLimitedBwe:
      RTC_LOG(LS_INFO) << "Not sending probe in bandwidth limited state. "
                       << (self.bandwidth_limited_cause) as isize;
      return {};
    case BandwidthLimitedCause::kLossLimitedBweIncreasing:
      max_probe_bitrate =
          std::cmp::min(max_probe_bitrate,
                   self.estimated_bitrate * self.config.loss_limited_probe_scale);
      break;
    case BandwidthLimitedCause::kDelayBasedLimited:
      break;
    default:
      break;
  }

  if (self.config.network_state_estimate_probing_interval->IsFinite() &&
      self.network_estimate && self.network_estimate.link_capacity_upper.IsFinite()) {
    if (self.network_estimate.link_capacity_upper.IsZero()) {
      RTC_LOG(LS_INFO) << "Not sending probe, Network state estimate is zero";
      return {};
    }
    max_probe_bitrate = std::cmp::min(
        {max_probe_bitrate,
         std::cmp::max(self.estimated_bitrate, self.network_estimate.link_capacity_upper *
                                          self.config.network_state_probe_scale)});
  }

  Vec<ProbeClusterConfig> pending_probes;
  for (DataRate bitrate : bitrates_to_probe) {
    assert!(!bitrate.IsZero());
    if (bitrate >= max_probe_bitrate) {
      bitrate = max_probe_bitrate;
      probe_further = false;
    }
    pending_probes.push_back(CreateProbeClusterConfig(now, bitrate));
  }
  self.time_last_probing_initiated = now;
  if (probe_further) {
    UpdateState(State::kWaitingForProbingResult);
    // Dont expect probe results to be larger than a fraction of the actual
    // probe rate.
    self.min_bitrate_to_probe_further = pending_probes.back().target_data_rate *
                                    self.config.further_probe_threshold;
  } else {
    UpdateState(State::kProbingComplete);
  }
  return pending_probes;
}

}  // namespace webrtc
