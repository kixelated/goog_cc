/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::{experiments::AlrExperimentSettings, pacing::IntervalBudget, rtc, FieldTrials};

#[derive(Clone, Debug)]
pub struct AlrDetectorConfig {
    // Sent traffic ratio as a function of network capacity used to determine
    // application-limited region. ALR region start when bandwidth usage drops
    // below AlrStartUsageRatio and ends when it raises above
    // AlrEndUsageRatio. NOTE: This is intentionally conservative at the moment
    // until BW adjustments of application limited region is fine tuned.
    pub bandwidth_usage_ratio: f64,
    pub start_budget_level_ratio: f64,
    pub stop_budget_level_ratio: f64,
}

impl Default for AlrDetectorConfig {
    fn default() -> Self {
        // The ALR experiment seems hard-coded on.
        // TODO Technically, WebRTC-AlrDetectorParameters could override these values.
        let settings = AlrExperimentSettings::default();
        Self {
            bandwidth_usage_ratio: settings.alr_bandwidth_usage_percent as f64 / 100.0,
            start_budget_level_ratio: settings.alr_start_budget_level_percent as f64 / 100.0,
            stop_budget_level_ratio: settings.alr_stop_budget_level_percent as f64 / 100.0,
        }
    }
}

// Application limited region detector is a class that utilizes signals of
// elapsed time and bytes sent to estimate whether network traffic is
// currently limited by the application's ability to generate traffic.
//
// AlrDetector provides a signal that can be utilized to adjust
// estimate bandwidth.
// Note: This class is not thread-safe.
pub struct AlrDetector {
    conf: AlrDetectorConfig,

    last_send_time_ms: Option<i64>,

    alr_budget: IntervalBudget,
    alr_started_time_ms: Option<i64>,
}

impl Default for AlrDetector {
    fn default() -> Self {
        Self::new(AlrDetectorConfig::default())
    }
}

impl AlrDetector {
    pub fn new(conf: AlrDetectorConfig) -> Self {
        Self {
            conf,
            last_send_time_ms: None,
            alr_budget: IntervalBudget::new(0, true),
            alr_started_time_ms: None,
        }
    }

    pub fn OnBytesSent(&mut self, bytes_sent: usize, send_time_ms: i64) {
        let last_send_time_ms = match self.last_send_time_ms {
            Some(v) => v,
            None => {
                // Since the duration for sending the bytes is unknwon, return without
                // updating alr state.
                self.last_send_time_ms = Some(send_time_ms);
                return;
            }
        };

        let delta_time_ms = send_time_ms - last_send_time_ms;
        self.last_send_time_ms = Some(send_time_ms);

        self.alr_budget.UseBudget(bytes_sent);
        self.alr_budget.IncreaseBudget(delta_time_ms);
        if self.alr_budget.budget_ratio() > self.conf.start_budget_level_ratio
            && self.alr_started_time_ms.is_none()
        {
            self.alr_started_time_ms.replace(rtc::TimeMillis());
        } else if self.alr_budget.budget_ratio() < self.conf.stop_budget_level_ratio
            && self.alr_started_time_ms.is_some()
        {
            self.alr_started_time_ms.take();
        }
    }

    // Set current estimated bandwidth.
    pub fn SetEstimatedBitrate(&mut self, bitrate_bps: i64) {
        assert!(bitrate_bps > 0);
        let target_rate_kbps: i64 =
            (bitrate_bps as f64 * self.conf.bandwidth_usage_ratio / 1000.0) as i64;
        self.alr_budget.set_target_rate_kbps(target_rate_kbps);
    }

    // Returns time in milliseconds when the current application-limited region
    // started or empty result if the sender is currently not application-limited.
    pub fn GetApplicationLimitedRegionStartTime(&self) -> Option<i64> {
        self.alr_started_time_ms
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const EstimatedBitrateBps: i64 = 300000;

    pub struct SimulateOutgoingTrafficIn<'a> {
        alr_detector: &'a mut AlrDetector,
        timestamp_ms: &'a mut i64,
        interval_ms: Option<i64>,
        usage_percentage: Option<i64>,
    }

    impl<'a> SimulateOutgoingTrafficIn<'a> {
        pub fn new(alr_detector: &'a mut AlrDetector, timestamp_ms: &'a mut i64) -> Self {
            Self {
                alr_detector,
                timestamp_ms,
                interval_ms: None,
                usage_percentage: None,
            }
        }

        pub fn ForTimeMs(mut self, time_ms: i64) -> Self {
            self.interval_ms.replace(time_ms);
            self
        }

        pub fn AtPercentOfEstimatedBitrate(mut self, usage_percentage: i64) {
            self.usage_percentage.replace(usage_percentage);
            self.ProduceTraffic();
        }

        fn ProduceTraffic(&mut self) {
            let interval_ms = self.interval_ms.unwrap();
            let usage_percentage = self.usage_percentage.unwrap();
            const TimeStepMs: i64 = 10;
            let mut t: i64 = 0;

            while t < interval_ms {
                *self.timestamp_ms += TimeStepMs;
                self.alr_detector.OnBytesSent(
                    (EstimatedBitrateBps * usage_percentage * TimeStepMs / (8 * 100 * 1000))
                        as usize,
                    *self.timestamp_ms,
                );
                t += TimeStepMs;
            }
            let remainder_ms: i64 = interval_ms % TimeStepMs;
            if remainder_ms > 0 {
                *self.timestamp_ms += TimeStepMs;
                self.alr_detector.OnBytesSent(
                    (EstimatedBitrateBps * usage_percentage * remainder_ms / (8 * 100 * 1000))
                        as usize,
                    *self.timestamp_ms,
                );
            }
        }
    }

    #[test]
    fn AlrDetection() {
        let mut timestamp_ms: i64 = 1000;
        let mut alr_detector = AlrDetector::default();
        alr_detector.SetEstimatedBitrate(EstimatedBitrateBps);

        // Start in non-ALR state.
        assert!(alr_detector
            .GetApplicationLimitedRegionStartTime()
            .is_none());

        // Stay in non-ALR state when usage is close to 100%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(1000)
            .AtPercentOfEstimatedBitrate(90);
        assert!(alr_detector
            .GetApplicationLimitedRegionStartTime()
            .is_none());

        // Verify that we ALR starts when bitrate drops below 20%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(1500)
            .AtPercentOfEstimatedBitrate(20);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_some())
        );

        // Verify that ALR ends when usage is above 65%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(4000)
            .AtPercentOfEstimatedBitrate(100);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );
    }

    #[test]
    fn ShortSpike() {
        let mut timestamp_ms: i64 = 1000;
        let mut alr_detector = AlrDetector::default();
        alr_detector.SetEstimatedBitrate(EstimatedBitrateBps);
        // Start in non-ALR state.
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );

        // Verify that we ALR starts when bitrate drops below 20%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(1000)
            .AtPercentOfEstimatedBitrate(20);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_some())
        );

        // Verify that we stay in ALR region even after a short bitrate spike.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(100)
            .AtPercentOfEstimatedBitrate(150);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_some())
        );

        // ALR ends when usage is above 65%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(3000)
            .AtPercentOfEstimatedBitrate(100);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );
    }

    #[test]
    fn BandwidthEstimateChanges() {
        let mut timestamp_ms: i64 = 1000;
        let mut alr_detector = AlrDetector::default();
        alr_detector.SetEstimatedBitrate(EstimatedBitrateBps);

        // Start in non-ALR state.
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );

        // ALR starts when bitrate drops below 20%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(1000)
            .AtPercentOfEstimatedBitrate(20);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_some())
        );

        // When bandwidth estimate drops the detector should stay in ALR mode and quit
        // it shortly afterwards as the sender continues sending the same amount of
        // traffic. This is necessary to ensure that ProbeController can still react
        // to the BWE drop by initiating a new probe.
        alr_detector.SetEstimatedBitrate(EstimatedBitrateBps / 5);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_some())
        );
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(1000)
            .AtPercentOfEstimatedBitrate(50);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );
    }

    #[test]
    fn ParseAlrSpecificFieldTrial() {
        let config = AlrDetectorConfig {
            bandwidth_usage_ratio: 0.90,
            start_budget_level_ratio: 0.0,
            stop_budget_level_ratio: -0.10,
        };
        let mut alr_detector = AlrDetector::new(config);
        let mut timestamp_ms: i64 = 1000;
        alr_detector.SetEstimatedBitrate(EstimatedBitrateBps);

        // Start in non-ALR state.
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );

        // ALR does not start at 100% utilization.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(1000)
            .AtPercentOfEstimatedBitrate(100);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_none())
        );

        // ALR does start at 85% utilization.
        // Overused 10% above so it should take about 2s to reach a budget level of
        // 0%.
        SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
            .ForTimeMs(2100)
            .AtPercentOfEstimatedBitrate(85);
        assert!(
            (alr_detector
                .GetApplicationLimitedRegionStartTime()
                .is_some())
        );
    }
}
