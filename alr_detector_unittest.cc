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
#include <optional>

#include "api/transport/field_trial_based_config.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/alr_experiment.h"
#include "test/field_trial.h"
#include "test/gtest.h"

namespace {

constexpr int kEstimatedBitrateBps = 300000;

}  // namespace


namespace {
pub struct SimulateOutgoingTrafficIn {
 public:
  explicit SimulateOutgoingTrafficIn(AlrDetector* alr_detector,
                                     i64* timestamp_ms)
      : alr_detector_(alr_detector), timestamp_ms_(timestamp_ms) {
    RTC_CHECK(self.alr_detector);
  }

fn ForTimeMs(int time_ms) -> SimulateOutgoingTrafficIn& {
    self.interval_ms = time_ms;
    ProduceTraffic();
    return *this;
  }

fn AtPercentOfEstimatedBitrate(int usage_percentage) -> SimulateOutgoingTrafficIn& {
    self.usage_percentage.emplace(usage_percentage);
    ProduceTraffic();
    return *this;
  }

 private:
fn ProduceTraffic() -> void {
    if (!interval_ms_ || !self.usage_percentage)
      return;
    const int kTimeStepMs = 10;
    for (int t = 0; t < *self.interval_ms; t += kTimeStepMs) {
      *self.timestamp_ms += kTimeStepMs;
      self.alr_detector.OnBytesSent(kEstimatedBitrateBps * *self.usage_percentage *
                                     kTimeStepMs / (8 * 100 * 1000),
                                 *self.timestamp_ms);
    }
    int remainder_ms = *interval_ms_ % kTimeStepMs;
    if (remainder_ms > 0) {
      *self.timestamp_ms += kTimeStepMs;
      self.alr_detector.OnBytesSent(kEstimatedBitrateBps * *self.usage_percentage *
                                     remainder_ms / (8 * 100 * 1000),
                                 *self.timestamp_ms);
    }
  }
  alr_detector: AlrDetector*,
  timestamp_ms: i64*,
  interval_ms: Option<int>,
  usage_percentage: Option<int>,
};
}  // namespace

#[test]
fn AlrDetection() {
  FieldTrialBasedConfig field_trials;
  i64 timestamp_ms = 1000;
  AlrDetector alr_detector(&field_trials);
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);

  // Start in non-ALR state.
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());

  // Stay in non-ALR state when usage is close to 100%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(90);
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());

  // Verify that we ALR starts when bitrate drops below 20%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(1500)
      .AtPercentOfEstimatedBitrate(20);
  EXPECT_TRUE(alr_detector.GetApplicationLimitedRegionStartTime());

  // Verify that ALR ends when usage is above 65%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(4000)
      .AtPercentOfEstimatedBitrate(100);
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());
}

#[test]
fn ShortSpike() {
  FieldTrialBasedConfig field_trials;
  i64 timestamp_ms = 1000;
  AlrDetector alr_detector(&field_trials);
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);
  // Start in non-ALR state.
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());

  // Verify that we ALR starts when bitrate drops below 20%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(20);
  EXPECT_TRUE(alr_detector.GetApplicationLimitedRegionStartTime());

  // Verify that we stay in ALR region even after a short bitrate spike.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(100)
      .AtPercentOfEstimatedBitrate(150);
  EXPECT_TRUE(alr_detector.GetApplicationLimitedRegionStartTime());

  // ALR ends when usage is above 65%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(3000)
      .AtPercentOfEstimatedBitrate(100);
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());
}

#[test]
fn BandwidthEstimateChanges() {
  FieldTrialBasedConfig field_trials;
  i64 timestamp_ms = 1000;
  AlrDetector alr_detector(&field_trials);
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);

  // Start in non-ALR state.
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());

  // ALR starts when bitrate drops below 20%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(20);
  EXPECT_TRUE(alr_detector.GetApplicationLimitedRegionStartTime());

  // When bandwidth estimate drops the detector should stay in ALR mode and quit
  // it shortly afterwards as the sender continues sending the same amount of
  // traffic. This is necessary to ensure that ProbeController can still react
  // to the BWE drop by initiating a new probe.
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps / 5);
  EXPECT_TRUE(alr_detector.GetApplicationLimitedRegionStartTime());
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(50);
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());
}

#[test]
fn ParseControlFieldTrial() {
  webrtc::test::ScopedFieldTrials scoped_field_trial(
      "WebRTC-ProbingScreenshareBwe/Control/");
  Option<AlrExperimentSettings> parsed_params =
      AlrExperimentSettings::CreateFromFieldTrial(
          FieldTrialBasedConfig(), "WebRTC-ProbingScreenshareBwe");
  EXPECT_FALSE(static_cast<bool>(parsed_params));
}

#[test]
fn ParseActiveFieldTrial() {
  webrtc::test::ScopedFieldTrials scoped_field_trial(
      "WebRTC-ProbingScreenshareBwe/1.1,2875,85,20,-20,1/");
  Option<AlrExperimentSettings> parsed_params =
      AlrExperimentSettings::CreateFromFieldTrial(
          FieldTrialBasedConfig(), "WebRTC-ProbingScreenshareBwe");
  ASSERT_TRUE(static_cast<bool>(parsed_params));
  EXPECT_EQ(1.1f, parsed_params->pacing_factor);
  EXPECT_EQ(2875, parsed_params->max_paced_queue_time);
  EXPECT_EQ(85, parsed_params->alr_bandwidth_usage_percent);
  EXPECT_EQ(20, parsed_params->alr_start_budget_level_percent);
  EXPECT_EQ(-20, parsed_params->alr_stop_budget_level_percent);
  EXPECT_EQ(1, parsed_params->group_id);
}

#[test]
fn ParseAlrSpecificFieldTrial() {
  webrtc::test::ScopedFieldTrials scoped_field_trial(
      "WebRTC-AlrDetectorParameters/"
      "bw_usage:90%,start:0%,stop:-10%/");
  FieldTrialBasedConfig field_trials;
  AlrDetector alr_detector(&field_trials);
  i64 timestamp_ms = 1000;
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);

  // Start in non-ALR state.
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());

  // ALR does not start at 100% utilization.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(100);
  EXPECT_FALSE(alr_detector.GetApplicationLimitedRegionStartTime());

  // ALR does start at 85% utilization.
  // Overused 10% above so it should take about 2s to reach a budget level of
  // 0%.
  SimulateOutgoingTrafficIn(&alr_detector, &timestamp_ms)
      .ForTimeMs(2100)
      .AtPercentOfEstimatedBitrate(85);
  EXPECT_TRUE(alr_detector.GetApplicationLimitedRegionStartTime());
}

}  // namespace webrtc
