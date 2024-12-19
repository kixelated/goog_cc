/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/alr_detector.h"

#include <cstdint>
#include <cstdio>
#include <memory>
#include <optional>

#include "api/field_trials_view.h"
#include "api/rtc_event_log/rtc_event_log.h"
#include "logging/rtc_event_log/events/rtc_event_alr_state.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/alr_experiment.h"
#include "rtc_base/experiments/struct_parameters_parser.h"
#include "rtc_base/time_utils.h"



namespace {
fn GetConfigFromTrials(const FieldTrialsView* key_value_config) -> AlrDetectorConfig {
  RTC_CHECK(AlrExperimentSettings::MaxOneFieldTrialEnabled(*key_value_config));
  Option<AlrExperimentSettings> experiment_settings =
      AlrExperimentSettings::CreateFromFieldTrial(
          *key_value_config,
          AlrExperimentSettings::kScreenshareProbingBweExperimentName);
  if (!experiment_settings) {
    experiment_settings = AlrExperimentSettings::CreateFromFieldTrial(
        *key_value_config,
        AlrExperimentSettings::kStrictPacingAndProbingExperimentName);
  }
  AlrDetectorConfig conf;
  if (experiment_settings) {
    conf.bandwidth_usage_ratio =
        experiment_settings->alr_bandwidth_usage_percent / 100.0;
    conf.start_budget_level_ratio =
        experiment_settings->alr_start_budget_level_percent / 100.0;
    conf.stop_budget_level_ratio =
        experiment_settings->alr_stop_budget_level_percent / 100.0;
  }
  conf.Parser()->Parse(
      key_value_config->Lookup("WebRTC-AlrDetectorParameters"));
  return conf;
}
}  //  namespace

std::unique_ptr<StructParametersParser> Parser(&self /* AlrDetectorConfig */) {
  return StructParametersParser::Create(   //
      "bw_usage", &bandwidth_usage_ratio,  //
      "start", &start_budget_level_ratio,  //
      "stop", &stop_budget_level_ratio);
}

AlrDetector::AlrDetector(AlrDetectorConfig config, RtcEventLog* event_log)
    : conf_(config), alr_budget_(0, true), event_log_(event_log) {}

AlrDetector::AlrDetector(const FieldTrialsView* key_value_config)
    : AlrDetector(GetConfigFromTrials(key_value_config), nullptr) {}

AlrDetector::AlrDetector(const FieldTrialsView* key_value_config,
                         RtcEventLog* event_log)
    : AlrDetector(GetConfigFromTrials(key_value_config), event_log) {}
AlrDetector::~AlrDetector() {}

fn OnBytesSent(&self /* AlrDetector */,usize bytes_sent, i64 send_time_ms) {
  if (!self.last_send_time_ms.is_some()) {
    self.last_send_time_ms = send_time_ms;
    // Since the duration for sending the bytes is unknwon, return without
    // updating alr state.
    return;
  }
  i64 delta_time_ms = send_time_ms - *self.last_send_time_ms;
  self.last_send_time_ms = send_time_ms;

  self.alr_budget.UseBudget(bytes_sent);
  self.alr_budget.IncreaseBudget(delta_time_ms);
  bool state_changed = false;
  if (self.alr_budget.budget_ratio() > self.conf.start_budget_level_ratio &&
      !self.alr_started_time_ms) {
    self.alr_started_time_ms.emplace(rtc::TimeMillis());
    state_changed = true;
  } else if (self.alr_budget.budget_ratio() < self.conf.stop_budget_level_ratio &&
             self.alr_started_time_ms) {
    state_changed = true;
    self.alr_started_time_ms.reset();
  }
  if (self.event_log && state_changed) {
    self.event_log.Log(
        std::make_unique<RtcEventAlrState>(self.alr_started_time_ms.is_some()));
  }
}

fn SetEstimatedBitrate(&self /* AlrDetector */,int bitrate_bps) {
  assert!(bitrate_bps);
  int target_rate_kbps =
      static_cast<f64>(bitrate_bps) * self.conf.bandwidth_usage_ratio / 1000;
  self.alr_budget.set_target_rate_kbps(target_rate_kbps);
}

Option<i64> GetApplicationLimitedRegionStartTime(&self /* AlrDetector */)
    {
  return self.alr_started_time_ms;
}

}  // namespace webrtc
