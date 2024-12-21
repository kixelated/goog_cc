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

#include <memory>
#include <optional>
#include <vector>

#include "absl/strings/string_view.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/mock/mock_rtc_event_log.h"
#include "system_wrappers/include/clock.h"
#include "test/explicit_key_value_config.h"
#include "test/gmock.h"
#include "test/gtest.h"

using ::testing::Gt;
using ::testing::IsEmpty;
using ::testing::NiceMock;
using ::testing::SizeIs;


namespace test {

namespace {

const MinBitrate: DataRate = DataRate::BitsPerSec(100);
const StartBitrate: DataRate = DataRate::BitsPerSec(300);
const MaxBitrate: DataRate = DataRate::BitsPerSec(10000);

const ExponentialProbingTimeout: TimeDelta = TimeDelta::Seconds(5);

const AlrProbeInterval: TimeDelta = TimeDelta::Seconds(5);
const AlrEndedTimeout: TimeDelta = TimeDelta::Seconds(3);
const BitrateDropTimeout: TimeDelta = TimeDelta::Seconds(5);
}  // namespace

pub struct ProbeControllerFixture {
 public:
  explicit ProbeControllerFixture(absl::string_view field_trials = "")
      : field_trial_config_(field_trials), clock_(100000000L) {}

fn CreateController() -> std::unique_ptr<ProbeController> {
    return std::make_unique<ProbeController>(&self.field_trial_config,
                                             &mock_rtc_event_log);
  }

fn CurrentTime() -> Timestamp { return self.clock.CurrentTime(); }
fn AdvanceTime(TimeDelta delta) { self.clock.AdvanceTime(delta); }

  field_trial_config: ExplicitKeyValueConfig,
  clock: SimulatedClock,
  NiceMock<MockRtcEventLog> mock_rtc_event_log;
};

#[test]
fn InitiatesProbingAfterSetBitrates() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  EXPECT_GE(probes.len(), 2u);
}

#[test]
fn InitiatesProbingWhenNetworkAvailable() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();

  Vec<ProbeClusterConfig> probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
  probes = probe_controller.OnNetworkAvailability({.network_available = true});
  EXPECT_GE(probes.len(), 2u);
}

#[test]
fn SetsDefaultTargetDurationAndTargetProbeCount() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  Vec<ProbeClusterConfig> probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_GE(probes.len(), 2u);

  assert_eq!(probes[0].target_duration, TimeDelta::Millis(15));
  assert_eq!(probes[0].target_probe_count, 5);
}

TEST(ProbeControllerTest,
     FieldTrialsOverrideDefaultTargetDurationAndTargetProbeCount) {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingBehavior/"
      "min_probe_packets_sent:2,min_probe_duration:123ms/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  Vec<ProbeClusterConfig> probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_GE(probes.len(), 2u);

  assert_eq!(probes[0].target_duration, TimeDelta::Millis(123));
  assert_eq!(probes[0].target_probe_count, 2);
}

#[test]
fn ProbeOnlyWhenNetworkIsUp() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
let probes = probe_controller.OnNetworkAvailability(
      {.at_time = fixture.CurrentTime(), .network_available = false});
  probes = probe_controller.SetBitrates(MinBitrate, StartBitrate,
                                         MaxBitrate, fixture.CurrentTime());
  assert!((probes.empty());
  probes = probe_controller.OnNetworkAvailability(
      {.at_time = fixture.CurrentTime(), .network_available = true});
  EXPECT_GE(probes.len(), 2u);
}

#[test]
fn CanConfigureInitialProbeRateFactor() {
  ProbeControllerFixture fixture("WebRTC-Bwe-ProbingConfiguration/p1:2,p2:3/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  assert_eq!(probes[0].target_data_rate, StartBitrate * 2);
  assert_eq!(probes[1].target_data_rate, StartBitrate * 3);
}

#[test]
fn DisableSecondInitialProbeIfRateFactorZero() {
  ProbeControllerFixture fixture("WebRTC-Bwe-ProbingConfiguration/p1:2,p2:0/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, StartBitrate * 2);
}

#[test]
fn InitiatesProbingOnMaxBitrateIncrease() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  // Long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate + DataRate::BitsPerSec(100),
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), MaxBitrate.bps() + 100);
}

#[test]
fn ProbesOnMaxAllocatedBitrateIncreaseOnlyWhenInAlr() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate - DataRate::BitsPerSec(1),
      BandwidthLimitedCause::DelayBasedLimited, fixture.CurrentTime());

  // Wait long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  // Probe when in alr.
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  probes = probe_controller.OnMaxTotalAllocatedBitrate(
      MaxBitrate + DataRate::BitsPerSec(1), fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  assert_eq!(probes.at(0).target_data_rate, MaxBitrate);

  // Do not probe when not in alr.
  probe_controller.SetAlrStartTimeMs(None);
  probes = probe_controller.OnMaxTotalAllocatedBitrate(
      MaxBitrate + DataRate::BitsPerSec(2), fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn ProbesOnMaxAllocatedBitrateLimitedByCurrentBwe() {
  ProbeControllerFixture fixture("");

  assert!(MaxBitrate > 1.5 * StartBitrate);
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  // Wait long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  // Probe when in alr.
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  probes = probe_controller.OnMaxTotalAllocatedBitrate(MaxBitrate,
                                                        fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes.at(0).target_data_rate, 2.0 * StartBitrate);

  // Continue probing if probe succeeds.
  probes = probe_controller.SetEstimatedBitrate(
      1.5 * StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  EXPECT_GT(probes.at(0).target_data_rate, 1.5 * StartBitrate);
}

#[test]
fn CanDisableProbingOnMaxTotalAllocatedBitrateIncrease() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "probe_max_allocation:false/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate - DataRate::BitsPerSec(1),
      BandwidthLimitedCause::DelayBasedLimited, fixture.CurrentTime());
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());

  // Do no probe, since probe_max_allocation:false.
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  probes = probe_controller.OnMaxTotalAllocatedBitrate(
      MaxBitrate + DataRate::BitsPerSec(1), fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn InitiatesProbingOnMaxBitrateIncreaseAtMaxBitrate() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  // Long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate + DataRate::BitsPerSec(100),
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate,
            MaxBitrate + DataRate::BitsPerSec(100));
}

#[test]
fn TestExponentialProbing() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());

  // Repeated probe should only be sent when estimated bitrate climbs above
  // 0.7 * 6 * StartBitrate = 1260.
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1000), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1800), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 2 * 1800);
}

#[test]
fn ExponentialProbingStopIfMaxBitrateLow() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/abort_further:true/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_THAT(probes, SizeIs(Gt(0)));

  // Repeated probe normally is sent when estimated bitrate climbs above
  // 0.7 * 6 * StartBitrate = 1260. But since max bitrate is low, expect
  // exponential probing to stop.
  probes = probe_controller.SetBitrates(MinBitrate, StartBitrate,
                                         /*max_bitrate=*/kStartBitrate,
                                         fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1800), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
}

#[test]
fn ExponentialProbingStopIfMaxAllocatedBitrateLow() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/abort_further:true/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_THAT(probes, SizeIs(Gt(0)));

  // Repeated probe normally is sent when estimated bitrate climbs above
  // 0.7 * 6 * StartBitrate = 1260. But since allocated bitrate i slow, expect
  // exponential probing to stop.
  probes = probe_controller.OnMaxTotalAllocatedBitrate(StartBitrate,
                                                        fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1800), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
}

#[test]
fn InitialProbingToLowMaxAllocatedbitrate() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_THAT(probes, SizeIs(Gt(0)));

  // Repeated probe is sent when estimated bitrate climbs above
  // 0.7 * 6 * StartBitrate = 1260.
  probes = probe_controller.OnMaxTotalAllocatedBitrate(StartBitrate,
                                                        fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());

  // If the inital probe result is received, a new probe is sent at 2x the
  // needed max bitrate.
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1800), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 2 * StartBitrate.bps());
}

#[test]
fn InitialProbingTimeout() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  EXPECT_THAT(probes, SizeIs(Gt(0)));
  // Advance far enough to cause a time out in waiting for probing result.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1800), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
}

#[test]
fn RepeatedInitialProbingSendsNewProbeAfterTimeout() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  probe_controller.EnableRepeatedInitialProbing(true);
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  EXPECT_THAT(probes, SizeIs(Gt(0)));
  let start_time: Timestamp = fixture.CurrentTime();
  let last_probe_time: Timestamp = fixture.CurrentTime();
  while (fixture.CurrentTime() < start_time + TimeDelta::Seconds(5)) {
    fixture.AdvanceTime(TimeDelta::Millis(100));
    probes = probe_controller.Process(fixture.CurrentTime());
    if (!probes.empty()) {
      // Expect a probe every second.
      assert_eq!(fixture.CurrentTime() - last_probe_time,
                TimeDelta::Seconds(1.1));
      assert_eq!(probes[0].min_probe_delta, TimeDelta::Millis(20));
      assert_eq!(probes[0].target_duration, TimeDelta::Millis(100));
      last_probe_time = fixture.CurrentTime();
    } else {
      EXPECT_LT(fixture.CurrentTime() - last_probe_time,
                TimeDelta::Seconds(1.1));
    }
  }
  fixture.AdvanceTime(TimeDelta::Seconds(1));
  // After 5s, repeated initial probing stops.
  EXPECT_THAT(probe_controller.Process(fixture.CurrentTime()), IsEmpty());
}

#[test]
fn RepeatedInitialProbingStopIfMaxAllocatedBitrateSet() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  probe_controller.EnableRepeatedInitialProbing(true);
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  EXPECT_THAT(probes, SizeIs(Gt(0)));

  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  EXPECT_THAT(probes, SizeIs(1));
  probes = probe_controller.OnMaxTotalAllocatedBitrate(MinBitrate,
                                                        fixture.CurrentTime());
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
}

#[test]
fn RequestProbeInAlr() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  EXPECT_GE(probes.len(), 2u);
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(250), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probes = probe_controller.RequestProbe(fixture.CurrentTime());

  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 0.85 * 500);
}

#[test]
fn RequestProbeWhenAlrEndedRecently() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  probe_controller.SetAlrStartTimeMs(None);
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(250), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probe_controller.SetAlrEndedTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(AlrEndedTimeout - TimeDelta::Millis(1));
  probes = probe_controller.RequestProbe(fixture.CurrentTime());

  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 0.85 * 500);
}

#[test]
fn RequestProbeWhenAlrNotEndedRecently() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  probe_controller.SetAlrStartTimeMs(None);
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(250), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probe_controller.SetAlrEndedTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(AlrEndedTimeout + TimeDelta::Millis(1));
  probes = probe_controller.RequestProbe(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn RequestProbeWhenBweDropNotRecent() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(250), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  fixture.AdvanceTime(BitrateDropTimeout + TimeDelta::Millis(1));
  probes = probe_controller.RequestProbe(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn PeriodicProbing() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  let start_time: Timestamp = fixture.CurrentTime();

  // Expect the controller to send a new probe after 5s has passed.
  probe_controller.SetAlrStartTimeMs(start_time.ms());
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 1000);

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  // The following probe should be sent at 10s into ALR.
  probe_controller.SetAlrStartTimeMs(start_time.ms());
  fixture.AdvanceTime(TimeDelta::Seconds(4));
  probes = probe_controller.Process(fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());

  probe_controller.SetAlrStartTimeMs(start_time.ms());
  fixture.AdvanceTime(TimeDelta::Seconds(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn PeriodicProbingAfterReset() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  let alr_start_time: Timestamp = fixture.CurrentTime();

  probe_controller.SetAlrStartTimeMs(alr_start_time.ms());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probe_controller.Reset(fixture.CurrentTime());

  fixture.AdvanceTime(TimeDelta::Seconds(10));
  probes = probe_controller.Process(fixture.CurrentTime());
  // Since bitrates are not yet set, no probe is sent event though we are in ALR
  // mode.
  assert!((probes.empty());

  probes = probe_controller.SetBitrates(MinBitrate, StartBitrate,
                                         MaxBitrate, fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);

  // Make sure we use `kStartBitrateBps` as the estimated bitrate
  // until SetEstimatedBitrate is called with an updated estimate.
  fixture.AdvanceTime(TimeDelta::Seconds(10));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, StartBitrate * 2);
}

#[test]
fn NoProbesWhenTransportIsNotWritable() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  probe_controller.EnablePeriodicAlrProbing(true);

  Vec<ProbeClusterConfig> probes =
      probe_controller.OnNetworkAvailability({.network_available = false});
  EXPECT_THAT(probes, IsEmpty());
  EXPECT_THAT(probe_controller.SetBitrates(MinBitrate, StartBitrate,
                                            MaxBitrate, fixture.CurrentTime()),
              IsEmpty());
  fixture.AdvanceTime(TimeDelta::Seconds(10));
  EXPECT_THAT(probe_controller.Process(fixture.CurrentTime()), IsEmpty());

  // Controller is reset after a network route change.
  // But, a probe should not be sent since the transport is not writable.
  // Transport is not writable until after DTLS negotiation completes.
  // However, the bitrate constraints may change.
  probe_controller.Reset(fixture.CurrentTime());
  EXPECT_THAT(
      probe_controller.SetBitrates(2 * MinBitrate, 2 * StartBitrate,
                                    2 * MaxBitrate, fixture.CurrentTime()),
      IsEmpty());
  fixture.AdvanceTime(TimeDelta::Seconds(10));
  EXPECT_THAT(probe_controller.Process(fixture.CurrentTime()), IsEmpty());
}

#[test]
fn TestExponentialProbingOverflow() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  const MbpsMultiplier: DataRate = DataRate::KilobitsPerSec(1000);
let probes = probe_controller.SetBitrates(MinBitrate, 10 * MbpsMultiplier,
                                              100 * MbpsMultiplier,
                                              fixture.CurrentTime());
  // Verify that probe bitrate is capped at the specified max bitrate.
  probes = probe_controller.SetEstimatedBitrate(
      60 * MbpsMultiplier, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, 100 * MbpsMultiplier);
  // Verify that repeated probes aren't sent.
  probes = probe_controller.SetEstimatedBitrate(
      100 * MbpsMultiplier, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn TestAllocatedBitrateCap() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  const MbpsMultiplier: DataRate = DataRate::KilobitsPerSec(1000);
  const MaxBitrate: DataRate = 100 * MbpsMultiplier;
let probes = probe_controller.SetBitrates(
      MinBitrate, 10 * MbpsMultiplier, MaxBitrate, fixture.CurrentTime());

  // Configure ALR for periodic probing.
  probe_controller.EnablePeriodicAlrProbing(true);
  let alr_start_time: Timestamp = fixture.CurrentTime();
  probe_controller.SetAlrStartTimeMs(alr_start_time.ms());

  let estimated_bitrate: DataRate = MaxBitrate / 10;
  probes = probe_controller.SetEstimatedBitrate(
      estimated_bitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  // Set a max allocated bitrate below the current estimate.
  let max_allocated: DataRate = estimated_bitrate - 1 * MbpsMultiplier;
  probes = probe_controller.OnMaxTotalAllocatedBitrate(max_allocated,
                                                        fixture.CurrentTime());
  assert!((probes.empty());  // No probe since lower than current max.

  // Probes such as ALR capped at 2x the max allocation limit.
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, 2 * max_allocated);

  // Remove allocation limit.
  assert!((
      probe_controller
          .OnMaxTotalAllocatedBitrate(DataRate::Zero(), fixture.CurrentTime())
          .empty());
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, estimated_bitrate * 2);
}

#[test]
fn ConfigurableProbingFieldTrial() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "p1:2,p2:5,step_size:3,further_probe_threshold:0.8,"
      "alloc_p1:2,alloc_current_bwe_limit:1000.0,alloc_p2,min_probe_packets_"
      "sent:2/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(MinBitrate, StartBitrate,
                                              DataRate::KilobitsPerSec(5000),
                                              fixture.CurrentTime());
  assert_eq!(probes.len(), 2u);
  assert_eq!(probes[0].target_data_rate.bps(), 600);
  assert_eq!(probes[0].target_probe_count, 2);
  assert_eq!(probes[1].target_data_rate.bps(), 1500);
  assert_eq!(probes[1].target_probe_count, 2);

  // Repeated probe should only be sent when estimated bitrate climbs above
  // 0.8 * 5 * StartBitrateBps = 1200.
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1100), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert_eq!(probes.len(), 0u);

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(1250), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 3 * 1250);

  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());

  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  probes = probe_controller.OnMaxTotalAllocatedBitrate(
      DataRate::KilobitsPerSec(200), fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate.bps(), 400_000);
}

#[test]
fn LimitAlrProbeWhenLossBasedBweLimited() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  // Expect the controller to send a new probe after 5s has passed.
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500),
      BandwidthLimitedCause::LossLimitedBweIncreasing, fixture.CurrentTime());
  fixture.AdvanceTime(TimeDelta::Seconds(6));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, 1.5 * DataRate::BitsPerSec(500));

  probes = probe_controller.SetEstimatedBitrate(
      1.5 * DataRate::BitsPerSec(500),
      BandwidthLimitedCause::DelayBasedLimited, fixture.CurrentTime());
  fixture.AdvanceTime(TimeDelta::Seconds(6));
  probes = probe_controller.Process(fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  EXPECT_GT(probes[0].target_data_rate, 1.5 * 1.5 * DataRate::BitsPerSec(500));
}

#[test]
fn PeriodicProbeAtUpperNetworkStateEstimate() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/network_state_interval:5s/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(5000), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  // Expect the controller to send a new probe after 5s has passed.
  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = DataRate::KilobitsPerSec(6);
  probe_controller.SetNetworkStateEstimate(state_estimate);

  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, state_estimate.link_capacity_upper);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, state_estimate.link_capacity_upper);
}

TEST(ProbeControllerTest,
     LimitProbeAtUpperNetworkStateEstimateIfLossBasedLimited) {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  // Expect the controller to send a new probe after 5s has passed.
  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = DataRate::BitsPerSec(700);
  probe_controller.SetNetworkStateEstimate(state_estimate);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);

  probes = probe_controller.SetEstimatedBitrate(
      DataRate::BitsPerSec(500),
      BandwidthLimitedCause::LossLimitedBweIncreasing, fixture.CurrentTime());
  // Expect the controller to send a new probe after 5s has passed.
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  assert_eq!(probes[0].target_data_rate, DataRate::BitsPerSec(700));
}

#[test]
fn AlrProbesLimitedByNetworkStateEstimate() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/network_state_interval:5s/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::KilobitsPerSec(6), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());

  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, MaxBitrate);

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = DataRate::BitsPerSec(8000);
  probe_controller.SetNetworkStateEstimate(state_estimate);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_data_rate, state_estimate.link_capacity_upper);
}

#[test]
fn CanSetLongerProbeDurationAfterNetworkStateEstimate() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s,network_state_probe_duration:100ms/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      DataRate::KilobitsPerSec(5), BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  EXPECT_LT(probes[0].target_duration, TimeDelta::Millis(100));

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = DataRate::KilobitsPerSec(6);
  probe_controller.SetNetworkStateEstimate(state_estimate);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes[0].target_duration, TimeDelta::Millis(100));
}

#[test]
fn ProbeInAlrIfLossBasedIncreasing() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probe_controller.EnablePeriodicAlrProbing(true);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::LossLimitedBweIncreasing,
      fixture.CurrentTime());

  // Wait long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  // Probe when in alr.
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  assert_eq!(probes.at(0).target_data_rate, 1.5 * StartBitrate);
}

#[test]
fn NotProbeWhenInAlrIfLossBasedDecreases() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probe_controller.EnablePeriodicAlrProbing(true);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::LossLimitedBwe,
      fixture.CurrentTime());

  // Wait long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  // Not probe in alr when loss based estimate decreases.
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn NotProbeIfLossBasedIncreasingOutsideAlr() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probe_controller.EnablePeriodicAlrProbing(true);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::LossLimitedBweIncreasing,
      fixture.CurrentTime());

  // Wait long enough to time out exponential probing.
  fixture.AdvanceTime(ExponentialProbingTimeout);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  probe_controller.SetAlrStartTimeMs(None);
  fixture.AdvanceTime(AlrProbeInterval + TimeDelta::Millis(1));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn ProbeFurtherWhenLossBasedIsSameAsDelayBasedEstimate() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = 5 * StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  let probe_target_rate: DataRate = probes[0].target_data_rate;
  EXPECT_LT(probe_target_rate, state_estimate.link_capacity_upper);
  // Expect that more probes are sent if BWE is the same as delay based
  // estimate.
  probes = probe_controller.SetEstimatedBitrate(
      probe_target_rate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  assert_eq!(probes[0].target_data_rate, 2 * probe_target_rate);
}

#[test]
fn ProbeIfEstimateLowerThanNetworkStateEstimate() {
  // Periodic probe every 1 second if estimate is lower than 50% of the
  // NetworkStateEstimate.
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/est_lower_than_network_interval:1s,"
      "est_lower_than_network_ratio:0.5,limit_probe_"
      "target_rate_to_loss_bwe:true/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  state_estimate.link_capacity_upper = StartBitrate * 3;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  probes = probe_controller.Process(fixture.CurrentTime());
  assert_eq!(probes.len(), 1u);
  EXPECT_GT(probes[0].target_data_rate, StartBitrate);

  // If network state not increased, send another probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(!(probes.empty());

  // Stop probing if estimate increase. We might probe further here though.
  probes = probe_controller.SetEstimatedBitrate(
      2 * StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  // No more periodic probes.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn DontProbeFurtherWhenLossLimited() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = 3 * StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(!(probes.empty());
  EXPECT_LT(probes[0].target_data_rate, state_estimate.link_capacity_upper);
  // Expect that no more probes are sent immediately if BWE is loss limited.
  probes = probe_controller.SetEstimatedBitrate(
      probes[0].target_data_rate, BandwidthLimitedCause::LossLimitedBwe,
      fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn ProbeFurtherWhenDelayBasedLimited() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = 3 * StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(!(probes.empty());
  EXPECT_LT(probes[0].target_data_rate, state_estimate.link_capacity_upper);
  // Since the probe was successfull, expect to continue probing.
  probes = probe_controller.SetEstimatedBitrate(
      probes[0].target_data_rate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!(!(probes.empty());
  assert_eq!(probes[0].target_data_rate, state_estimate.link_capacity_upper);
}

TEST(ProbeControllerTest,
     ProbeAfterTimeoutIfNetworkStateEstimateIncreaseAfterProbeSent) {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s,est_lower_than_network_interval:3s,est_lower_"
      "than_network_ratio:0.7/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = 1.2 * probes[0].target_data_rate / 2;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  // No immediate further probing since probe result is low.
  probes = probe_controller.SetEstimatedBitrate(
      probes[0].target_data_rate / 2, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!(probes.empty());

  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  EXPECT_LE(probes[0].target_data_rate, state_estimate.link_capacity_upper);
  // If the network state estimate increase, even before the probe result,
  // expect a new probe after `est_lower_than_network_interval` timeout.
  state_estimate.link_capacity_upper = 3 * StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  probes = probe_controller.SetEstimatedBitrate(
      probes[0].target_data_rate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());

  fixture.AdvanceTime(TimeDelta::Seconds(3));
  probes = probe_controller.Process(fixture.CurrentTime());
  EXPECT_THAT(probes, Not(IsEmpty()));

  // But no more probes if estimate is close to the link capacity.
  probes = probe_controller.SetEstimatedBitrate(
      state_estimate.link_capacity_upper * 0.9,
      BandwidthLimitedCause::DelayBasedLimited, fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn SkipProbeFurtherIfAlreadyProbedToMaxRate() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:2s,skip_if_est_larger_than_fraction_of_max:0.9/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  probe_controller.SetNetworkStateEstimate(
      {.link_capacity_upper = 2 * MaxBitrate});

  // Attempt to probe up to max rate.
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate * 0.8, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  assert_eq!(probes[0].target_data_rate, MaxBitrate);

  // If the probe result arrives, dont expect a new probe immediately since we
  // already tried to probe at the max rate.
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate * 0.8, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());

  fixture.AdvanceTime(TimeDelta::Millis(1000));
  probes = probe_controller.Process(fixture.CurrentTime());
  EXPECT_THAT(probes, IsEmpty());
  // But when enough time has passed, expect a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1000));
  probes = probe_controller.Process(fixture.CurrentTime());
  EXPECT_THAT(probes, Not(IsEmpty()));
}

#[test]
fn MaxAllocatedBitrateNotReset() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  probes = probe_controller.OnMaxTotalAllocatedBitrate(StartBitrate / 4,
                                                        fixture.CurrentTime());
  probe_controller.Reset(fixture.CurrentTime());

  probes = probe_controller.SetBitrates(MinBitrate, StartBitrate,
                                         MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  assert_eq!(probes[0].target_data_rate, StartBitrate / 4 * 2);
}

#[test]
fn SkipAlrProbeIfEstimateLargerThanMaxProbe() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "skip_if_est_larger_than_fraction_of_max:0.9/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());

  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  fixture.AdvanceTime(TimeDelta::Seconds(10));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  // But if the max rate increase, A new probe is sent.
  probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, 2 * MaxBitrate, fixture.CurrentTime());
  assert!(!(probes.empty());
}

TEST(ProbeControllerTest,
     SkipAlrProbeIfEstimateLargerThanFractionOfMaxAllocated) {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "skip_if_est_larger_than_fraction_of_max:1.0/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate / 2, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());

  fixture.AdvanceTime(TimeDelta::Seconds(10));
  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  probes = probe_controller.OnMaxTotalAllocatedBitrate(MaxBitrate / 2,
                                                        fixture.CurrentTime());
  // No probes since total allocated is not higher than the current estimate.
  assert!((probes.empty());
  fixture.AdvanceTime(TimeDelta::Seconds(2));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());

  // But if the max allocated increase, A new probe is sent.
  probes = probe_controller.OnMaxTotalAllocatedBitrate(
      MaxBitrate / 2 + DataRate::BitsPerSec(1), fixture.CurrentTime());
  assert!(!(probes.empty());
}

#[test]
fn SkipNetworkStateProbeIfEstimateLargerThanMaxProbe() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:2s,skip_if_est_larger_than_fraction_of_max:0.9/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  probe_controller.SetNetworkStateEstimate(
      {.link_capacity_upper = 2 * MaxBitrate});
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());

  fixture.AdvanceTime(TimeDelta::Seconds(10));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn SendsProbeIfNetworkStateEstimateLowerThanMaxProbe() {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:2s,skip_if_est_larger_than_fraction_of_max:0.9,"
      "network_state_probe_duration:100ms,network_"
      "state_min_probe_delta:20/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  probe_controller.SetNetworkStateEstimate(
      {.link_capacity_upper = 2 * MaxBitrate});
  probes = probe_controller.SetEstimatedBitrate(
      MaxBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());

  // Need to wait at least two seconds before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(2100));

  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  assert!((probes.empty());
  probe_controller.SetNetworkStateEstimate(
      {.link_capacity_upper = 2 * StartBitrate});
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(!(probes.empty());
  EXPECT_LE(probes[0].target_data_rate, 2 * StartBitrate);
  // Expect probe durations to be picked from field trial probe target is lower
  // or equal to the network state estimate.
  assert_eq!(probes[0].min_probe_delta, TimeDelta::Millis(20));
  assert_eq!(probes[0].target_duration, TimeDelta::Millis(100));
}

TEST(ProbeControllerTest,
     ProbeNotLimitedByNetworkStateEsimateIfLowerThantCurrent) {
  ProbeControllerFixture fixture(
      "WebRTC-Bwe-ProbingConfiguration/"
      "network_state_interval:5s,network_state_probe_duration:100ms,network_"
      "state_min_probe_delta:20/");
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());
  probe_controller.EnablePeriodicAlrProbing(true);
let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimited,
      fixture.CurrentTime());
  probe_controller.SetNetworkStateEstimate(
      {.link_capacity_upper = StartBitrate});
  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  probe_controller.SetAlrStartTimeMs(fixture.CurrentTime().ms());
  probe_controller.SetNetworkStateEstimate(
      {.link_capacity_upper = StartBitrate / 2});
  fixture.AdvanceTime(TimeDelta::Seconds(6));
  probes = probe_controller.Process(fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());
  assert_eq!(probes[0].target_data_rate, StartBitrate);
  // Expect probe durations to be default since network state estimate is lower
  // than the probe rate.
  assert_eq!(probes[0].min_probe_delta, TimeDelta::Millis(2));
  assert_eq!(probes[0].target_duration, TimeDelta::Millis(15));
}

#[test]
fn DontProbeIfDelayIncreased() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = 3 * StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::DelayBasedLimitedDelayIncreased,
      fixture.CurrentTime());
  assert!(probes.empty());

  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());
}

#[test]
fn DontProbeIfHighRtt() {
  ProbeControllerFixture fixture;
  std::unique_ptr<ProbeController> probe_controller =
      fixture.CreateController();
  ASSERT_THAT(
      probe_controller.OnNetworkAvailability({.network_available = true}),
      IsEmpty());

let probes = probe_controller.SetBitrates(
      MinBitrate, StartBitrate, MaxBitrate, fixture.CurrentTime());
  ASSERT_FALSE(probes.empty());

  // Need to wait at least one second before process can trigger a new probe.
  fixture.AdvanceTime(TimeDelta::Millis(1100));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!(probes.empty());

  NetworkStateEstimate state_estimate;
  state_estimate.link_capacity_upper = 3 * StartBitrate;
  probe_controller.SetNetworkStateEstimate(state_estimate);
  probes = probe_controller.SetEstimatedBitrate(
      StartBitrate, BandwidthLimitedCause::RttBasedBackOffHighRtt,
      fixture.CurrentTime());
  assert!(probes.empty());

  fixture.AdvanceTime(TimeDelta::Seconds(5));
  probes = probe_controller.Process(fixture.CurrentTime());
  assert!((probes.empty());
}
}  // namespace test
}  // namespace webrtc
