/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */



pub struct AlrDetectorConfig {
  // Sent traffic ratio as a function of network capacity used to determine
  // application-limited region. ALR region start when bandwidth usage drops
  // below kAlrStartUsageRatio and ends when it raises above
  // kAlrEndUsageRatio. NOTE: This is intentionally conservative at the moment
  // until BW adjustments of application limited region is fine tuned.
  pub bandwidth_usage_ratio: f64,
  pub start_budget_level_ratio: f64,
  pub stop_budget_level_ratio: f64,
}

impl Default for AlrDetectorConfig {
  fn default() -> Self {
      Self {
          bandwidth_usage_ratio: 0.65,
          start_budget_level_ratio: 0.80,
          stop_budget_level_ratio: 0.50,
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
#[derive(Default)]
pub struct AlrDetector {
  conf: AlrDetectorConfig,

  last_send_time_ms: Option<i64>,

  alr_budget: IntervalBudget,
  alr_started_time_ms: Option<i64>,
}

impl AlrDetector {
  pub fn new(conf: AlrDetectorConfig) -> Self {
    Self {
      conf,
      last_send_time_ms: None,
      alr_budget: Default::default(), // (0, true)
      alr_started_time_ms: None,
    }
  }

  pub fn OnBytesSent(&self, bytes_sent: usize, send_time_ms: i64) {
    if (!self.last_send_time_ms.is_some()) {
      self.last_send_time_ms = Some(send_time_ms);
      // Since the duration for sending the bytes is unknwon, return without
      // updating alr state.
      return;
    }

    let delta_time_ms= send_time_ms - *self.last_send_time_ms;
    self.last_send_time_ms = Some(send_time_ms);

    self.alr_budget.UseBudget(bytes_sent);
    self.alr_budget.IncreaseBudget(delta_time_ms);
    let state_changed= false;
    if (self.alr_budget.budget_ratio() > self.conf.start_budget_level_ratio &&
        !self.alr_started_time_ms) {
      self.alr_started_time_ms.replace(rtc::TimeMillis());
      state_changed = true;
    } else if (self.alr_budget.budget_ratio() < self.conf.stop_budget_level_ratio &&
               self.alr_started_time_ms.is_some()) {
      state_changed = true;
      self.alr_started_time_ms.take();
    }
  }

  // Set current estimated bandwidth.
  pub fn SetEstimatedBitrate(&mut self, bitrate_bps: isize) {
    assert!(bitrate_bps > 0);
    let target_rate_kbps: isize =
        (bitrate_bps as f64) * self.conf.bandwidth_usage_ratio / 1000;
    self.alr_budget.set_target_rate_kbps(target_rate_kbps);
  }

  // Returns time in milliseconds when the current application-limited region
  // started or empty result if the sender is currently not application-limited.
  pub fn GetApplicationLimitedRegionStartTime(&self) -> Option<i64> {
    return self.alr_started_time_ms;
  }
}


#[cfg(test)]
mod test {
  use super::*;

const kEstimatedBitrateBps: isize = 300000;

pub struct SimulateOutgoingTrafficIn<'a> {
  alr_detector: &'a mut AlrDetector,
  timestamp_ms: &'a mut i64,
  interval_ms: Option<isize>,
  usage_percentage: Option<isize>,
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


pub fn ForTimeMs(mut self, time_ms: isize) -> Self {
    self.interval_ms.replace(time_ms);
    return self
  }

pub fn AtPercentOfEstimatedBitrate(mut self, usage_percentage: isize) {
    self.usage_percentage.replace(usage_percentage);
    self.ProduceTraffic();
  }

fn ProduceTraffic(&mut self) {
  let interval_ms= self.interval_ms.unwrap();
  let usage_percentage= self.usage_percentage.unwrap();
    const kTimeStepMs: isize = 10;
    let t: isize = 0;

    while t < interval_ms {
      *self.timestamp_ms += kTimeStepMs;
      self.alr_detector.OnBytesSent(kEstimatedBitrateBps * usage_percentage *
                                     kTimeStepMs / (8 * 100 * 1000),
                                 *self.timestamp_ms);
                                 t += kTimeStepMs;
    }
    let remainder_ms: isize = self.interval_ms % kTimeStepMs;
    if (remainder_ms > 0) {
      *self.timestamp_ms += kTimeStepMs;
      self.alr_detector.OnBytesSent(kEstimatedBitrateBps * usage_percentage *
                                     remainder_ms / (8 * 100 * 1000),
                                 *self.timestamp_ms);
    }
  }
}

#[test]
fn AlrDetection() {
  let mut timestamp_ms: i64 = 1000;
  let mut alr_detector = AlrDetector::default();
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);

  // Start in non-ALR state.
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));

  // Stay in non-ALR state when usage is close to 100%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(90);
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));

  // Verify that we ALR starts when bitrate drops below 20%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(1500)
      .AtPercentOfEstimatedBitrate(20);
  assert!((alr_detector.GetApplicationLimitedRegionStartTime()));

  // Verify that ALR ends when usage is above 65%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(4000)
      .AtPercentOfEstimatedBitrate(100);
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));
}

#[test]
fn ShortSpike() {
  let timestamp_ms: i64 = 1000;
  let mut alr_detector = AlrDetector::default();
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);
  // Start in non-ALR state.
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));

  // Verify that we ALR starts when bitrate drops below 20%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(20);
  assert!((alr_detector.GetApplicationLimitedRegionStartTime()));

  // Verify that we stay in ALR region even after a short bitrate spike.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(100)
      .AtPercentOfEstimatedBitrate(150);
  assert!((alr_detector.GetApplicationLimitedRegionStartTime()));

  // ALR ends when usage is above 65%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(3000)
      .AtPercentOfEstimatedBitrate(100);
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));
}

#[test]
fn BandwidthEstimateChanges() {
  let timestamp_ms: i64 = 1000;
  let mut alr_detector = AlrDetector::default();
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);

  // Start in non-ALR state.
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));

  // ALR starts when bitrate drops below 20%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(20);
  assert!((alr_detector.GetApplicationLimitedRegionStartTime()));

  // When bandwidth estimate drops the detector should stay in ALR mode and quit
  // it shortly afterwards as the sender continues sending the same amount of
  // traffic. This is necessary to ensure that ProbeController can still react
  // to the BWE drop by initiating a new probe.
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps / 5);
  assert!((alr_detector.GetApplicationLimitedRegionStartTime()));
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(50);
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));
}


#[test]
fn ParseAlrSpecificFieldTrial() {
      let config= AlrDetectorConfig {
        bandwidth_usage_ratio: 0.90,
        start_budget_level_ratio: 0.0,
        stop_budget_level_ratio: -0.10,
      };
  let mut alr_detector = AlrDetector::new(config);
  let timestamp_ms: i64 = 1000;
  alr_detector.SetEstimatedBitrate(kEstimatedBitrateBps);

  // Start in non-ALR state.
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));

  // ALR does not start at 100% utilization.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(1000)
      .AtPercentOfEstimatedBitrate(100);
  assert!(!(alr_detector.GetApplicationLimitedRegionStartTime()));

  // ALR does start at 85% utilization.
  // Overused 10% above so it should take about 2s to reach a budget level of
  // 0%.
  SimulateOutgoingTrafficIn::new(&mut alr_detector, &mut timestamp_ms)
      .ForTimeMs(2100)
      .AtPercentOfEstimatedBitrate(85);
  assert!((alr_detector.GetApplicationLimitedRegionStartTime()));
}
}