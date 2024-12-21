/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <queue>
#include <string>
#include <utility>
#include <vector>

#include "absl/strings/string_view.h"
#include "api/environment/environment.h"
#include "api/environment/environment_factory.h"
#include "api/test/network_emulation/create_cross_traffic.h"
#include "api/test/network_emulation/cross_traffic.h"
#include "api/transport/goog_cc_factory.h"
#include "api/transport/network_control.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "call/video_receive_stream.h"
#include "logging/rtc_event_log/mock/mock_rtc_event_log.h"
#include "test/field_trial.h"
#include "test/gtest.h"
#include "test/scenario/call_client.h"
#include "test/scenario/column_printer.h"
#include "test/scenario/scenario.h"
#include "test/scenario/scenario_config.h"

using ::testing::IsEmpty;
using ::testing::NiceMock;


namespace test {
namespace {
// Count dips from a constant high bandwidth level within a short window.
int CountBandwidthDips(std::queue<DataRate> bandwidth_history,
                       threshold: DataRate) {
  if (bandwidth_history.empty())
    return true;
  let first: DataRate = bandwidth_history.front();
  bandwidth_history.pop();

  let dips: isize = 0;
  let state_high: bool = true;
  while (!bandwidth_history.empty()) {
    if (bandwidth_history.front() + threshold < first && state_high) {
      dips += 1;
      state_high = false;
    } else if (bandwidth_history.front() == first) {
      state_high = true;
    } else if (bandwidth_history.front() > first) {
      // If this is toggling we will catch it later when front becomes first.
      state_high = false;
    }
    bandwidth_history.pop();
  }
  return dips;
}
GoogCcNetworkControllerFactory CreateFeedbackOnlyFactory() {
  GoogCcFactoryConfig config;
  config.feedback_only = true;
  return GoogCcNetworkControllerFactory(std::move(config));
}

const InitialBitrateKbps: u32 = 60;
const InitialBitrate: DataRate = DataRate::KilobitsPerSec(InitialBitrateKbps);
const DefaultPacingRate: f32 = 2.5f;

CallClient* CreateVideoSendingClient(
    Scenario* s,
    CallClientConfig config,
    Vec<EmulatedNetworkNode*> send_link,
    Vec<EmulatedNetworkNode*> return_link) {
let client = s.CreateClient("send", std::move(config));
let route = s.CreateRoutes(client, send_link,
                                s.CreateClient("return", CallClientConfig()),
                                return_link);
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  return client;
}

NetworkRouteChange CreateRouteChange(
    time: Timestamp,
    start_rate: Option<DataRate> = None,
    min_rate: Option<DataRate> = None,
    max_rate: Option<DataRate> = None) {
  NetworkRouteChange route_change;
  route_change.at_time = time;
  route_change.constraints.at_time = time;
  route_change.constraints.min_data_rate = min_rate;
  route_change.constraints.max_data_rate = max_rate;
  route_change.constraints.starting_rate = start_rate;
  return route_change;
}

PacketResult CreatePacketResult(arrival_time: Timestamp,
                                send_time: Timestamp,
                                usize payload_size,
                                PacedPacketInfo pacing_info) {
  PacketResult packet_result;
  packet_result.sent_packet = SentPacket();
  packet_result.sent_packet.send_time = send_time;
  packet_result.sent_packet.size = DataSize::Bytes(payload_size);
  packet_result.sent_packet.pacing_info = pacing_info;
  packet_result.receive_time = arrival_time;
  return packet_result;
}

// Simulate sending packets and receiving transport feedback during
// `runtime_ms`, then return the final target birate.
PacketTransmissionAndFeedbackBlock: Option<DataRate>(
    NetworkControllerInterface* controller,
    i64 runtime_ms,
    i64 delay,
    Timestamp& current_time) {
  NetworkControlUpdate update;
  target_bitrate: Option<DataRate>;
  let delay_buildup: i64 = 0;
  let start_time_ms: i64 = current_time.ms();
  while (current_time.ms() - start_time_ms < runtime_ms) {
    const PayloadSize: usize = 1000;
    let packet: PacketResult =
        CreatePacketResult(current_time + TimeDelta::Millis(delay_buildup),
                           current_time, PayloadSize, PacedPacketInfo());
    delay_buildup += delay;
    update = controller.OnSentPacket(packet.sent_packet);
    if (update.target_rate) {
      target_bitrate = update.target_rate.target_rate;
    }
    TransportPacketsFeedback feedback;
    feedback.feedback_time = packet.receive_time;
    feedback.packet_feedbacks.push_back(packet);
    update = controller.OnTransportPacketsFeedback(feedback);
    if (update.target_rate) {
      target_bitrate = update.target_rate.target_rate;
    }
    current_time += TimeDelta::Millis(50);
    update = controller.OnProcessInterval({.at_time = current_time});
    if (update.target_rate) {
      target_bitrate = update.target_rate.target_rate;
    }
  }
  return target_bitrate;
}

// Create transport packets feedback with a built-up delay.
TransportPacketsFeedback CreateTransportPacketsFeedback(
    TimeDelta per_packet_network_delay,
    TimeDelta one_way_delay,
    send_time: Timestamp) {
  let delay_buildup: TimeDelta = one_way_delay;
  const FeedbackSize: isize = 3;
  const PayloadSize: usize = 1000;
  TransportPacketsFeedback feedback;
  for (isize i = 0; i < FeedbackSizei += 1) {
    let packet: PacketResult = CreatePacketResult(
        /*arrival_time=*/send_time + delay_buildup, send_time, PayloadSize,
        PacedPacketInfo());
    delay_buildup += per_packet_network_delay;
    feedback.feedback_time = packet.receive_time + one_way_delay;
    feedback.packet_feedbacks.push_back(packet);
  }
  return feedback;
}

// Scenarios:

fn UpdatesTargetRateBasedOnLinkCapacity(absl::string_view test_name = "") {
let factory = CreateFeedbackOnlyFactory();
  Scenario s("googcc_unit/target_capacity" + std::string(test_name), false);
  CallClientConfig config;
  config.transport.cc_factory = &factory;
  config.transport.rates.min_rate = DataRate::KilobitsPerSec(10);
  config.transport.rates.max_rate = DataRate::KilobitsPerSec(1500);
  config.transport.rates.start_rate = DataRate::KilobitsPerSec(300);
let send_net = s.CreateMutableSimulationNode([](NetworkSimulationConfig* c) {
    c.bandwidth = DataRate::KilobitsPerSec(500);
    c.delay = TimeDelta::Millis(100);
    c.loss_rate = 0.0;
  });
let ret_net = s.CreateMutableSimulationNode(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(100); });
let truth: &StatesPrinter = s.CreatePrinter(
      "send.truth.txt", TimeDelta::PlusInfinity(), {send_net.ConfigPrinter()});

let client = CreateVideoSendingClient(&s, config, {send_net.node()},
                                          {ret_net.node()});

  truth.PrintRow();
  s.RunFor(TimeDelta::Seconds(25));
  truth.PrintRow();
  assert_relative_eq!(client.target_rate().kbps(), 450, 100);

  send_net.UpdateConfig([](NetworkSimulationConfig* c) {
    c.bandwidth = DataRate::KilobitsPerSec(800);
    c.delay = TimeDelta::Millis(100);
  });

  truth.PrintRow();
  s.RunFor(TimeDelta::Seconds(20));
  truth.PrintRow();
  assert_relative_eq!(client.target_rate().kbps(), 750, 150);

  send_net.UpdateConfig([](NetworkSimulationConfig* c) {
    c.bandwidth = DataRate::KilobitsPerSec(100);
    c.delay = TimeDelta::Millis(200);
  });
  ret_net.UpdateConfig(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(200); });

  truth.PrintRow();
  s.RunFor(TimeDelta::Seconds(50));
  truth.PrintRow();
  assert_relative_eq!(client.target_rate().kbps(), 90, 25);
}

fn RunRembDipScenario(absl::string_view test_name) -> DataRate {
  Scenario s(test_name);
  NetworkSimulationConfig net_conf;
  net_conf.bandwidth = DataRate::KilobitsPerSec(2000);
  net_conf.delay = TimeDelta::Millis(50);
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = DataRate::KilobitsPerSec(1000);
  });
let send_net = {s.CreateSimulationNode(net_conf)};
let ret_net = {s.CreateSimulationNode(net_conf)};
let route = s.CreateRoutes(
      client, send_net, s.CreateClient("return", CallClientConfig()), ret_net);
  s.CreateVideoStream(route.forward(), VideoStreamConfig());

  s.RunFor(TimeDelta::Seconds(10));
  EXPECT_GT(client.send_bandwidth().kbps(), 1500);

  let RembLimit: DataRate = DataRate::KilobitsPerSec(250);
  client.SetRemoteBitrate(RembLimit);
  s.RunFor(TimeDelta::Seconds(1));
  assert_eq!(client.send_bandwidth(), RembLimit);

  let RembLimitLifted: DataRate = DataRate::KilobitsPerSec(10000);
  client.SetRemoteBitrate(RembLimitLifted);
  s.RunFor(TimeDelta::Seconds(10));

  return client.send_bandwidth();
}

}  // namespace

pub struct NetworkControllerTestFixture {
 public:
  NetworkControllerTestFixture() : factory_() {}
  explicit NetworkControllerTestFixture(GoogCcFactoryConfig googcc_config)
      : factory_(std::move(googcc_config)) {}

fn CreateController() -> std::unique_ptr<NetworkControllerInterface> {
    let config: NetworkControllerConfig = InitialConfig();
    std::unique_ptr<NetworkControllerInterface> controller =
        self.factory.Create(config);
    return controller;
  }

 private:
  NetworkControllerConfig InitialConfig(
      let starting_bandwidth_kbps: isize = InitialBitrateKbps,
      let min_data_rate_kbps: isize = 0,
      let max_data_rate_kbps: isize = 5 * InitialBitrateKbps) {
    NetworkControllerConfig config(self.env);
    config.constraints.at_time = Timestamp::Zero();
    config.constraints.min_data_rate =
        DataRate::KilobitsPerSec(min_data_rate_kbps);
    config.constraints.max_data_rate =
        DataRate::KilobitsPerSec(max_data_rate_kbps);
    config.constraints.starting_rate =
        DataRate::KilobitsPerSec(starting_bandwidth_kbps);
    return config;
  }

  event_log: NiceMock<MockRtcEventLog>,
  const Environment self.env = CreateEnvironment(&self.event_log);
  factory: GoogCcNetworkControllerFactory,
};

TEST(GoogCcNetworkControllerTest,
     InitializeTargetRateOnFirstProcessIntervalAfterNetworkAvailable) {
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();

  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = Timestamp::Millis(123456), .network_available = true});
  update =
      controller.OnProcessInterval({.at_time = Timestamp::Millis(123456)});

  assert_eq!(update.target_rate.target_rate, InitialBitrate);
  assert_eq!(update.pacer_config.data_rate(),
            InitialBitrate * DefaultPacingRate);
  assert_eq!(update.probe_cluster_configs[0].target_data_rate,
            InitialBitrate * 3);
  assert_eq!(update.probe_cluster_configs[1].target_data_rate,
            InitialBitrate * 5);
}

#[test]
fn ReactsToChangedNetworkConditions() {
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);
  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});
  update = controller.OnProcessInterval({.at_time = current_time});
  update = controller.OnRemoteBitrateReport(
      {.receive_time = current_time, .bandwidth = InitialBitrate * 2});

  current_time += TimeDelta::Millis(25);
  update = controller.OnProcessInterval({.at_time = current_time});
  assert_eq!(update.target_rate.target_rate, InitialBitrate * 2);
  assert_eq!(update.pacer_config.data_rate(),
            InitialBitrate * 2 * DefaultPacingRate);

  update = controller.OnRemoteBitrateReport(
      {.receive_time = current_time, .bandwidth = InitialBitrate});
  current_time += TimeDelta::Millis(25);
  update = controller.OnProcessInterval({.at_time = current_time});
  assert_eq!(update.target_rate.target_rate, InitialBitrate);
  assert_eq!(update.pacer_config.data_rate(),
            InitialBitrate * DefaultPacingRate);
}

#[test]
fn OnNetworkRouteChanged() {
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);
  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});
  let new_bitrate: DataRate = DataRate::BitsPerSec(200000);

  update = controller.OnNetworkRouteChange(
      CreateRouteChange(current_time, new_bitrate));
  assert_eq!(update.target_rate.target_rate, new_bitrate);
  assert_eq!(update.pacer_config.data_rate(), new_bitrate * DefaultPacingRate);
  assert_eq!(update.probe_cluster_configs.len(), 2u);

  // If the bitrate is reset to -1, the new starting bitrate will be
  // the minimum default bitrate.
  const DefaultMinBitrate: DataRate = DataRate::KilobitsPerSec(5);
  update = controller.OnNetworkRouteChange(CreateRouteChange(current_time));
  assert_eq!(update.target_rate.target_rate, DefaultMinBitrate);
  assert_relative_eq!(update.pacer_config.data_rate().bps<f64>(),
              DefaultMinBitrate.bps<f64>() * DefaultPacingRate, 10);
  assert_eq!(update.probe_cluster_configs.len(), 2u);
}

#[test]
fn ProbeOnRouteChange() {
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);
  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});
  current_time += TimeDelta::Seconds(3);

  update = controller.OnNetworkRouteChange(
      CreateRouteChange(current_time, 2 * InitialBitrate, DataRate::Zero(),
                        20 * InitialBitrate));

  assert!((update.pacer_config.is_some());
  assert_eq!(update.target_rate.target_rate, InitialBitrate * 2);
  assert_eq!(update.probe_cluster_configs.len(), 2u);
  assert_eq!(update.probe_cluster_configs[0].target_data_rate,
            InitialBitrate * 6);
  assert_eq!(update.probe_cluster_configs[1].target_data_rate,
            InitialBitrate * 12);

  update = controller.OnProcessInterval({.at_time = current_time});
}

#[test]
fn ProbeAfterRouteChangeWhenTransportWritable() {
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);

  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = false});
  EXPECT_THAT(update.probe_cluster_configs, IsEmpty());

  update = controller.OnNetworkRouteChange(
      CreateRouteChange(current_time, 2 * InitialBitrate, DataRate::Zero(),
                        20 * InitialBitrate));
  // Transport is not writable. So not point in sending a probe.
  EXPECT_THAT(update.probe_cluster_configs, IsEmpty());

  // Probe is sent when transport becomes writable.
  update = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});
  EXPECT_THAT(update.probe_cluster_configs, Not(IsEmpty()));
}

// Bandwidth estimation is updated when feedbacks are received.
// Feedbacks which show an increasing delay cause the estimation to be reduced.
#[test]
fn UpdatesDelayBasedEstimate() {
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  const RunTimeMs: i64 = 6000;
  let current_time: Timestamp = Timestamp::Millis(123);
  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});

  // The test must run and insert packets/feedback long enough that the
  // BWE computes a valid estimate. This is first done in an environment which
  // simulates no bandwidth limitation, and therefore not built-up delay.
  target_bitrate_before_delay: Option<DataRate> =
      PacketTransmissionAndFeedbackBlock(controller.get(), RunTimeMs, 0,
                                         current_time);
  assert!(target_bitrate_before_delay.is_some());

  // Repeat, but this time with a building delay, and make sure that the
  // estimation is adjusted downwards.
  target_bitrate_after_delay: Option<DataRate> =
      PacketTransmissionAndFeedbackBlock(controller.get(), RunTimeMs, 50,
                                         current_time);
  EXPECT_LT(*target_bitrate_after_delay, *target_bitrate_before_delay);
}

#[test]
fn PaceAtMaxOfLowerLinkCapacityAndBwe() {
  ScopedFieldTrials trial(
      "WebRTC-Bwe-PaceAtMaxOfBweAndLowerLinkCapacity/Enabled/");
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);
  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});
  update = controller.OnProcessInterval({.at_time = current_time});
  current_time += TimeDelta::Millis(100);
  let network_estimate: NetworkStateEstimate = {.link_capacity_lower =
                                               10 * InitialBitrate};
  update = controller.OnNetworkStateEstimate(network_estimate);
  // OnNetworkStateEstimate does not trigger processing a new estimate. So add a
  // dummy loss report to trigger a BWE update in the next process interval.
  TransportLossReport loss_report;
  loss_report.start_time = current_time;
  loss_report.end_time = current_time;
  loss_report.receive_time = current_time;
  loss_report.packets_received_delta = 50;
  loss_report.packets_lost_delta = 1;
  update = controller.OnTransportLossReport(loss_report);
  update = controller.OnProcessInterval({.at_time = current_time});
  assert!(update.pacer_config);
  assert!(update.target_rate);
  ASSERT_LT(update.target_rate.target_rate,
            network_estimate.link_capacity_lower);
  assert_eq!(update.pacer_config.data_rate().kbps(),
            network_estimate.link_capacity_lower.kbps() * DefaultPacingRate);

  current_time += TimeDelta::Millis(100);
  // Set a low link capacity estimate and verify that pacing rate is set
  // relative to loss based/delay based estimate.
  network_estimate = {.link_capacity_lower = 0.5 * InitialBitrate};
  update = controller.OnNetworkStateEstimate(network_estimate);
  // Again, we need to inject a dummy loss report to trigger an update of the
  // BWE in the next process interval.
  loss_report.start_time = current_time;
  loss_report.end_time = current_time;
  loss_report.receive_time = current_time;
  loss_report.packets_received_delta = 50;
  loss_report.packets_lost_delta = 0;
  update = controller.OnTransportLossReport(loss_report);
  update = controller.OnProcessInterval({.at_time = current_time});
  assert!(update.target_rate);
  ASSERT_GT(update.target_rate.target_rate,
            network_estimate.link_capacity_lower);
  assert_eq!(update.pacer_config.data_rate().kbps(),
            update.target_rate.target_rate.kbps() * DefaultPacingRate);
}

#[test]
fn LimitPacingFactorToUpperLinkCapacity() {
  ScopedFieldTrials trial(
      "WebRTC-Bwe-LimitPacingFactorByUpperLinkCapacityEstimate/Enabled/");
  NetworkControllerTestFixture fixture;
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);
  let update: NetworkControlUpdate = controller.OnNetworkAvailability(
      {.at_time = current_time, .network_available = true});
  update = controller.OnProcessInterval({.at_time = current_time});
  current_time += TimeDelta::Millis(100);
  let network_estimate: NetworkStateEstimate = {
      .link_capacity_upper = InitialBitrate * DefaultPacingRate / 2};
  update = controller.OnNetworkStateEstimate(network_estimate);
  // OnNetworkStateEstimate does not trigger processing a new estimate. So add a
  // dummy loss report to trigger a BWE update in the next process interval.
  TransportLossReport loss_report;
  loss_report.start_time = current_time;
  loss_report.end_time = current_time;
  loss_report.receive_time = current_time;
  loss_report.packets_received_delta = 50;
  loss_report.packets_lost_delta = 1;
  update = controller.OnTransportLossReport(loss_report);
  update = controller.OnProcessInterval({.at_time = current_time});
  assert!(update.pacer_config);
  assert!(update.target_rate);
  EXPECT_GE(update.target_rate.target_rate, InitialBitrate);
  assert_eq!(update.pacer_config.data_rate(),
            network_estimate.link_capacity_upper);
}

// Test congestion window pushback on network delay happens.
#[test]
fn CongestionWindowPushbackOnNetworkDelay() {
let factory = CreateFeedbackOnlyFactory();
  ScopedFieldTrials trial(
      "WebRTC-CongestionWindow/QueueSize:800,MinBitrate:30000/");
  Scenario s("googcc_unit/cwnd_on_delay", false);
let send_net =
      s.CreateMutableSimulationNode([=](NetworkSimulationConfig* c) {
        c.bandwidth = DataRate::KilobitsPerSec(1000);
        c.delay = TimeDelta::Millis(100);
      });
let ret_net = s.CreateSimulationNode(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(100); });
  CallClientConfig config;
  config.transport.cc_factory = &factory;
  // Start high so bandwidth drop has max effect.
  config.transport.rates.start_rate = DataRate::KilobitsPerSec(300);
  config.transport.rates.max_rate = DataRate::KilobitsPerSec(2000);
  config.transport.rates.min_rate = DataRate::KilobitsPerSec(10);

let client = CreateVideoSendingClient(&s, std::move(config),
                                          {send_net.node()}, {ret_net});

  s.RunFor(TimeDelta::Seconds(10));
  send_net.PauseTransmissionUntil(s.Now() + TimeDelta::Seconds(10));
  s.RunFor(TimeDelta::Seconds(3));

  // After 3 seconds without feedback from any sent packets, we expect that the
  // target rate is reduced to the minimum pushback threshold
  // DefaultMinPushbackTargetBitrateBps, which is defined as 30 kbps in
  // congestion_window_pushback_controller.
  EXPECT_LT(client.target_rate().kbps(), 40);
}

// Test congestion window pushback on network delay happens.
#[test]
fn CongestionWindowPushbackDropFrameOnNetworkDelay() {
let factory = CreateFeedbackOnlyFactory();
  ScopedFieldTrials trial(
      "WebRTC-CongestionWindow/QueueSize:800,MinBitrate:30000,DropFrame:true/");
  Scenario s("googcc_unit/cwnd_on_delay", false);
let send_net =
      s.CreateMutableSimulationNode([=](NetworkSimulationConfig* c) {
        c.bandwidth = DataRate::KilobitsPerSec(1000);
        c.delay = TimeDelta::Millis(100);
      });
let ret_net = s.CreateSimulationNode(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(100); });
  CallClientConfig config;
  config.transport.cc_factory = &factory;
  // Start high so bandwidth drop has max effect.
  config.transport.rates.start_rate = DataRate::KilobitsPerSec(300);
  config.transport.rates.max_rate = DataRate::KilobitsPerSec(2000);
  config.transport.rates.min_rate = DataRate::KilobitsPerSec(10);

let client = CreateVideoSendingClient(&s, std::move(config),
                                          {send_net.node()}, {ret_net});

  s.RunFor(TimeDelta::Seconds(10));
  send_net.PauseTransmissionUntil(s.Now() + TimeDelta::Seconds(10));
  s.RunFor(TimeDelta::Seconds(3));

  // As the dropframe is set, after 3 seconds without feedback from any sent
  // packets, we expect that the target rate is not reduced by congestion
  // window.
  EXPECT_GT(client.target_rate().kbps(), 300);
}

#[test]
fn PaddingRateLimitedByCongestionWindowInTrial() {
  ScopedFieldTrials trial(
      "WebRTC-CongestionWindow/QueueSize:200,MinBitrate:30000/");

  Scenario s("googcc_unit/padding_limited", false);
let send_net =
      s.CreateMutableSimulationNode([=](NetworkSimulationConfig* c) {
        c.bandwidth = DataRate::KilobitsPerSec(1000);
        c.delay = TimeDelta::Millis(100);
      });
let ret_net = s.CreateSimulationNode(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(100); });
  CallClientConfig config;
  // Start high so bandwidth drop has max effect.
  config.transport.rates.start_rate = DataRate::KilobitsPerSec(1000);
  config.transport.rates.max_rate = DataRate::KilobitsPerSec(2000);
let client = s.CreateClient("send", config);
let route =
      s.CreateRoutes(client, {send_net.node()},
                     s.CreateClient("return", CallClientConfig()), {ret_net});
  VideoStreamConfig video;
  video.stream.pad_to_rate = config.transport.rates.max_rate;
  s.CreateVideoStream(route.forward(), video);

  // Run for a few seconds to allow the controller to stabilize.
  s.RunFor(TimeDelta::Seconds(10));

  // Check that padding rate matches target rate.
  assert_relative_eq!(client.padding_rate().kbps(), client.target_rate().kbps(), 1);

  // Check this is also the case when congestion window pushback kicks in.
  send_net.PauseTransmissionUntil(s.Now() + TimeDelta::Seconds(1));
  assert_relative_eq!(client.padding_rate().kbps(), client.target_rate().kbps(), 1);
}

#[test]
fn LimitsToFloorIfRttIsHighInTrial() {
  // The field trial limits maximum RTT to 2 seconds, higher RTT means that the
  // controller backs off until it reaches the minimum configured bitrate. This
  // allows the RTT to recover faster than the regular control mechanism would
  // achieve.
  const BandwidthFloor: DataRate = DataRate::KilobitsPerSec(50);
  ScopedFieldTrials trial("WebRTC-Bwe-MaxRttLimit/limit:2s,floor:" +
                          std::to_string(BandwidthFloor.kbps()) + "kbps/");
  // In the test case, we limit the capacity and add a cross traffic packet
  // burst that blocks media from being sent. This causes the RTT to quickly
  // increase above the threshold in the trial.
  const LinkCapacity: DataRate = DataRate::KilobitsPerSec(100);
  const BufferBloatDuration: TimeDelta = TimeDelta::Seconds(10);
  Scenario s("googcc_unit/limit_trial", false);
let send_net = s.CreateSimulationNode([=](NetworkSimulationConfig* c) {
    c.bandwidth = LinkCapacity;
    c.delay = TimeDelta::Millis(100);
  });
let ret_net = s.CreateSimulationNode(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(100); });
  CallClientConfig config;
  config.transport.rates.start_rate = LinkCapacity;

let client = CreateVideoSendingClient(&s, config, {send_net}, {ret_net});
  // Run for a few seconds to allow the controller to stabilize.
  s.RunFor(TimeDelta::Seconds(10));
  const BloatPacketSize: DataSize = DataSize::Bytes(1000);
  const BloatPacketCount: isize =
      (BufferBloatDuration * LinkCapacity / BloatPacketSize) as isize;
  // This will cause the RTT to be large for a while.
  s.TriggerPacketBurst({send_net}, BloatPacketCount, BloatPacketSize.bytes());
  // Wait to allow the high RTT to be detected and acted upon.
  s.RunFor(TimeDelta::Seconds(6));
  // By now the target rate should have dropped to the minimum configured rate.
  assert_relative_eq!(client.target_rate().kbps(), BandwidthFloor.kbps(), 5);
}

#[test]
fn UpdatesTargetRateBasedOnLinkCapacity() {
  UpdatesTargetRateBasedOnLinkCapacity();
}

#[test]
fn StableEstimateDoesNotVaryInSteadyState() {
let factory = CreateFeedbackOnlyFactory();
  Scenario s("googcc_unit/stable_target", false);
  CallClientConfig config;
  config.transport.cc_factory = &factory;
  NetworkSimulationConfig net_conf;
  net_conf.bandwidth = DataRate::KilobitsPerSec(500);
  net_conf.delay = TimeDelta::Millis(100);
let send_net = s.CreateSimulationNode(net_conf);
let ret_net = s.CreateSimulationNode(net_conf);

let client = CreateVideoSendingClient(&s, config, {send_net}, {ret_net});
  // Run for a while to allow the estimate to stabilize.
  s.RunFor(TimeDelta::Seconds(30));
  let min_stable_target: DataRate = DataRate::PlusInfinity();
  let max_stable_target: DataRate = DataRate::MinusInfinity();
  let min_target: DataRate = DataRate::PlusInfinity();
  let max_target: DataRate = DataRate::MinusInfinity();

  // Measure variation in steady state.
  for (isize i = 0; i < 20i += 1) {
let stable_target_rate = client.stable_target_rate();
let target_rate = client.target_rate();
    EXPECT_LE(stable_target_rate, target_rate);

    min_stable_target = std::cmp::min(min_stable_target, stable_target_rate);
    max_stable_target = std::cmp::max(max_stable_target, stable_target_rate);
    min_target = std::cmp::min(min_target, target_rate);
    max_target = std::cmp::max(max_target, target_rate);
    s.RunFor(TimeDelta::Seconds(1));
  }
  // We should expect drops by at least 15% (default backoff.)
  EXPECT_LT(min_target / max_target, 0.85);
  // We should expect the stable target to be more stable than the immediate one
  EXPECT_GE(min_stable_target / max_stable_target, min_target / max_target);
}

#[test]
fn LossBasedControlUpdatesTargetRateBasedOnLinkCapacity() {
  ScopedFieldTrials trial("WebRTC-Bwe-LossBasedControl/Enabled/");
  // TODO(srte): Should the behavior be unaffected at low loss rates?
  UpdatesTargetRateBasedOnLinkCapacity("_loss_based");
}

#[test]
fn LossBasedControlDoesModestBackoffToHighLoss() {
  ScopedFieldTrials trial("WebRTC-Bwe-LossBasedControl/Enabled/");
  Scenario s("googcc_unit/high_loss_channel", false);
  CallClientConfig config;
  config.transport.rates.min_rate = DataRate::KilobitsPerSec(10);
  config.transport.rates.max_rate = DataRate::KilobitsPerSec(1500);
  config.transport.rates.start_rate = DataRate::KilobitsPerSec(300);
let send_net = s.CreateSimulationNode([](NetworkSimulationConfig* c) {
    c.bandwidth = DataRate::KilobitsPerSec(2000);
    c.delay = TimeDelta::Millis(200);
    c.loss_rate = 0.1;
  });
let ret_net = s.CreateSimulationNode(
      [](NetworkSimulationConfig* c) { c.delay = TimeDelta::Millis(200); });

let client = CreateVideoSendingClient(&s, config, {send_net}, {ret_net});

  s.RunFor(TimeDelta::Seconds(120));
  // Without LossBasedControl trial, bandwidth drops to ~10 kbps.
  EXPECT_GT(client.target_rate().kbps(), 100);
}

fn AverageBitrateAfterCrossInducedLoss(absl::string_view name) -> DataRate {
  Scenario s(name, false);
  NetworkSimulationConfig net_conf;
  net_conf.bandwidth = DataRate::KilobitsPerSec(1000);
  net_conf.delay = TimeDelta::Millis(100);
  // Short queue length means that we'll induce loss when sudden TCP traffic
  // spikes are induced. This corresponds to ca 200 ms for a packet size of 1000
  // bytes. Such limited buffers are common on for instance wifi routers.
  net_conf.packet_queue_length_limit = 25;

let send_net = {s.CreateSimulationNode(net_conf)};
let ret_net = {s.CreateSimulationNode(net_conf)};

let client = s.CreateClient("send", CallClientConfig());
let callee = s.CreateClient("return", CallClientConfig());
let route = s.CreateRoutes(client, send_net, callee, ret_net);
  // TODO(srte): Make this work with RTX enabled or remove it.
let video = s.CreateVideoStream(route.forward(), [](VideoStreamConfig* c) {
    c.stream.use_rtx = false;
  });
  s.RunFor(TimeDelta::Seconds(10));
  for (isize i = 0; i < 4i += 1) {
    // Sends TCP cross traffic inducing loss.
let tcp_traffic = s.net().StartCrossTraffic(CreateFakeTcpCrossTraffic(
        s.net().CreateRoute(send_net), s.net().CreateRoute(ret_net),
        FakeTcpConfig()));
    s.RunFor(TimeDelta::Seconds(2));
    // Allow the ccongestion controller to recover.
    s.net().StopCrossTraffic(tcp_traffic);
    s.RunFor(TimeDelta::Seconds(20));
  }

  // Querying the video stats from within the expected runtime environment
  // (i.e. the TQ that belongs to the CallClient, not the Scenario TQ that
  // we're currently on).
  VideoReceiveStreamInterface::Stats video_receive_stats;
let video_stream = video.receive();
  callee.SendTask([&video_stream, &video_receive_stats]() {
    video_receive_stats = video_stream.GetStats();
  });
  return DataSize::Bytes(
             video_receive_stats.rtp_stats.packet_counter.TotalBytes()) /
         s.TimeSinceStart();
}

#[test]
fn MaintainsLowRateInSafeResetTrial() {
  const LinkCapacity: DataRate = DataRate::KilobitsPerSec(200);
  const StartRate: DataRate = DataRate::KilobitsPerSec(300);

  ScopedFieldTrials trial("WebRTC-Bwe-SafeResetOnRouteChange/Enabled/");
  Scenario s("googcc_unit/safe_reset_low");
let send_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = LinkCapacity;
    c.delay = TimeDelta::Millis(10);
  });
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = StartRate;
  });
let route = s.CreateRoutes(
      client, {send_net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow the controller to stabilize.
  s.RunFor(TimeDelta::Millis(500));
  assert_relative_eq!(client.send_bandwidth().kbps(), LinkCapacity.kbps(), 50);
  s.ChangeRoute(route.forward(), {send_net});
  // Allow new settings to propagate.
  s.RunFor(TimeDelta::Millis(100));
  // Under the trial, the target should be unchanged for low rates.
  assert_relative_eq!(client.send_bandwidth().kbps(), LinkCapacity.kbps(), 50);
}

#[test]
fn DoNotResetBweUnlessNetworkAdapterChangeOnRoutChange() {
  ScopedFieldTrials trial("WebRTC-Bwe-ResetOnAdapterIdChange/Enabled/");
  Scenario s("googcc_unit/do_not_reset_bwe_unless_adapter_change");

  const LinkCapacity: DataRate = DataRate::KilobitsPerSec(1000);
  const StartRate: DataRate = DataRate::KilobitsPerSec(300);

let send_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = LinkCapacity;
    c.delay = TimeDelta::Millis(50);
  });
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = StartRate;
  });
  client.UpdateNetworkAdapterId(0);
let route = s.CreateRoutes(
      client, {send_net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow the controller to stabilize.
  s.RunFor(TimeDelta::Millis(500));
  assert_relative_eq!(client.send_bandwidth().kbps(), LinkCapacity.kbps(), 300);
  s.ChangeRoute(route.forward(), {send_net});
  // Allow new settings to propagate.
  s.RunFor(TimeDelta::Millis(50));
  // Under the trial, the target should not drop.
  assert_relative_eq!(client.send_bandwidth().kbps(), LinkCapacity.kbps(), 300);

  s.RunFor(TimeDelta::Millis(500));
  // But if adapter id change, BWE should reset and start from the beginning if
  // the network route changes.
  client.UpdateNetworkAdapterId(1);
  s.ChangeRoute(route.forward(), {send_net});
  // Allow new settings to propagate.
  s.RunFor(TimeDelta::Millis(50));
  assert_relative_eq!(client.send_bandwidth().kbps(), StartRate.kbps(), 30);
}

#[test]
fn CutsHighRateInSafeResetTrial() {
  const LinkCapacity: DataRate = DataRate::KilobitsPerSec(1000);
  const StartRate: DataRate = DataRate::KilobitsPerSec(300);

  ScopedFieldTrials trial("WebRTC-Bwe-SafeResetOnRouteChange/Enabled/");
  Scenario s("googcc_unit/safe_reset_high_cut");
let send_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = LinkCapacity;
    c.delay = TimeDelta::Millis(50);
  });
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = StartRate;
  });
let route = s.CreateRoutes(
      client, {send_net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow the controller to stabilize.
  s.RunFor(TimeDelta::Millis(500));
  assert_relative_eq!(client.send_bandwidth().kbps(), LinkCapacity.kbps(), 300);
  client.UpdateNetworkAdapterId(1);
  s.ChangeRoute(route.forward(), {send_net});
  // Allow new settings to propagate.
  s.RunFor(TimeDelta::Millis(50));
  // Under the trial, the target should be reset from high values.
  assert_relative_eq!(client.send_bandwidth().kbps(), StartRate.kbps(), 30);
}

#[test]
fn DetectsHighRateInSafeResetTrial() {
  ScopedFieldTrials trial("WebRTC-Bwe-SafeResetOnRouteChange/Enabled,ack/");
  const InitialLinkCapacity: DataRate = DataRate::KilobitsPerSec(200);
  const NewLinkCapacity: DataRate = DataRate::KilobitsPerSec(800);
  const StartRate: DataRate = DataRate::KilobitsPerSec(300);

  Scenario s("googcc_unit/safe_reset_high_detect");
let initial_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = InitialLinkCapacity;
    c.delay = TimeDelta::Millis(50);
  });
let new_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = NewLinkCapacity;
    c.delay = TimeDelta::Millis(50);
  });
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = StartRate;
  });
let route = s.CreateRoutes(
      client, {initial_net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow the controller to stabilize.
  s.RunFor(TimeDelta::Millis(2000));
  assert_relative_eq!(client.send_bandwidth().kbps(), InitialLinkCapacity.kbps(), 50);
  client.UpdateNetworkAdapterId(1);
  s.ChangeRoute(route.forward(), {new_net});
  // Allow new settings to propagate, but not probes to be received.
  s.RunFor(TimeDelta::Millis(50));
  // Under the field trial, the target rate should be unchanged since it's lower
  // than the starting rate.
  assert_relative_eq!(client.send_bandwidth().kbps(), InitialLinkCapacity.kbps(), 50);
  // However, probing should have made us detect the higher rate.
  // NOTE: This test causes high loss rate, and the loss-based estimator reduces
  // the bitrate, making the test fail if we wait longer than one second here.
  s.RunFor(TimeDelta::Millis(1000));
  EXPECT_GT(client.send_bandwidth().kbps(), NewLinkCapacity.kbps() - 300);
}

#[test]
fn TargetRateReducedOnPacingBufferBuildupInTrial() {
  // Configure strict pacing to ensure build-up.
  ScopedFieldTrials trial(
      "WebRTC-CongestionWindow/QueueSize:100,MinBitrate:30000/"
      "WebRTC-Video-Pacing/factor:1.0/"
      "WebRTC-AddPacingToCongestionWindowPushback/Enabled/");

  const LinkCapacity: DataRate = DataRate::KilobitsPerSec(1000);
  const StartRate: DataRate = DataRate::KilobitsPerSec(1000);

  Scenario s("googcc_unit/pacing_buffer_buildup");
let net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = LinkCapacity;
    c.delay = TimeDelta::Millis(50);
  });
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = StartRate;
  });
let route = s.CreateRoutes(
      client, {net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow some time for the buffer to build up.
  s.RunFor(TimeDelta::Seconds(5));

  // Without trial, pacer delay reaches ~250 ms.
  EXPECT_LT(client.GetStats().pacer_delay_ms, 150);
}

#[test]
fn NoBandwidthTogglingInLossControlTrial() {
  ScopedFieldTrials trial("WebRTC-Bwe-LossBasedControl/Enabled/");
  Scenario s("googcc_unit/no_toggling");
let send_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = DataRate::KilobitsPerSec(2000);
    c.loss_rate = 0.2;
    c.delay = TimeDelta::Millis(10);
  });

let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = DataRate::KilobitsPerSec(300);
  });
let route = s.CreateRoutes(
      client, {send_net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow the controller to initialize.
  s.RunFor(TimeDelta::Millis(250));

  std::queue<DataRate> bandwidth_history;
  const step: TimeDelta = TimeDelta::Millis(50);
  for (TimeDelta time = TimeDelta::Zero(); time < TimeDelta::Millis(2000);
       time += step) {
    s.RunFor(step);
    const window: TimeDelta = TimeDelta::Millis(500);
    if (bandwidth_history.len() >= window / step)
      bandwidth_history.pop();
    bandwidth_history.push(client.send_bandwidth());
    EXPECT_LT(
        CountBandwidthDips(bandwidth_history, DataRate::KilobitsPerSec(100)),
        2);
  }
}

#[test]
fn NoRttBackoffCollapseWhenVideoStops() {
  ScopedFieldTrials trial("WebRTC-Bwe-MaxRttLimit/limit:2s/");
  Scenario s("googcc_unit/rttbackoff_video_stop");
let send_net = s.CreateSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = DataRate::KilobitsPerSec(2000);
    c.delay = TimeDelta::Millis(100);
  });

let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = DataRate::KilobitsPerSec(1000);
  });
let route = s.CreateRoutes(
      client, {send_net}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});
let video = s.CreateVideoStream(route.forward(), VideoStreamConfig());
  // Allow the controller to initialize, then stop video.
  s.RunFor(TimeDelta::Seconds(1));
  video.send().Stop();
  s.RunFor(TimeDelta::Seconds(4));
  EXPECT_GT(client.send_bandwidth().kbps(), 1000);
}

#[test]
fn NoCrashOnVeryLateFeedback() {
  Scenario s;
let ret_net = s.CreateMutableSimulationNode(NetworkSimulationConfig());
let route = s.CreateRoutes(
      s.CreateClient("send", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())},
      s.CreateClient("return", CallClientConfig()), {ret_net.node()});
let video = s.CreateVideoStream(route.forward(), VideoStreamConfig());
  s.RunFor(TimeDelta::Seconds(5));
  // Delay feedback by several minutes. This will cause removal of the send time
  // history for the packets as long as SendTimeHistoryWindow is configured for
  // a shorter time span.
  ret_net.PauseTransmissionUntil(s.Now() + TimeDelta::Seconds(300));
  // Stopping video stream while waiting to save test execution time.
  video.send().Stop();
  s.RunFor(TimeDelta::Seconds(299));
  // Starting to cause addition of new packet to history, which cause old
  // packets to be removed.
  video.send().Start();
  // Runs until the lost packets are received. We expect that this will run
  // without causing any runtime failures.
  s.RunFor(TimeDelta::Seconds(2));
}

#[test]
fn IsFairToTCP() {
  Scenario s("googcc_unit/tcp_fairness");
  NetworkSimulationConfig net_conf;
  net_conf.bandwidth = DataRate::KilobitsPerSec(1000);
  net_conf.delay = TimeDelta::Millis(50);
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = DataRate::KilobitsPerSec(1000);
  });
let send_net = {s.CreateSimulationNode(net_conf)};
let ret_net = {s.CreateSimulationNode(net_conf)};
let route = s.CreateRoutes(
      client, send_net, s.CreateClient("return", CallClientConfig()), ret_net);
  s.CreateVideoStream(route.forward(), VideoStreamConfig());
  s.net().StartCrossTraffic(CreateFakeTcpCrossTraffic(
      s.net().CreateRoute(send_net), s.net().CreateRoute(ret_net),
      FakeTcpConfig()));
  s.RunFor(TimeDelta::Seconds(10));

  // Currently only testing for the upper limit as we in practice back out
  // quite a lot in this scenario. If this behavior is fixed, we should add a
  // lower bound to ensure it stays fixed.
  EXPECT_LT(client.send_bandwidth().kbps(), 750);
}

#[test]
fn FastRampupOnRembCapLifted() {
  let final_estimate: DataRate =
      RunRembDipScenario("googcc_unit/default_fast_rampup_on_remb_cap_lifted");
  EXPECT_GT(final_estimate.kbps(), 1500);
}

#[test]
fn FallbackToLossBasedBweWithoutPacketFeedback() {
  const LinkCapacity: DataRate = DataRate::KilobitsPerSec(1000);
  const StartRate: DataRate = DataRate::KilobitsPerSec(1000);

  Scenario s("googcc_unit/high_loss_channel", false);
let net = s.CreateMutableSimulationNode([&](NetworkSimulationConfig* c) {
    c.bandwidth = LinkCapacity;
    c.delay = TimeDelta::Millis(100);
  });
let client = s.CreateClient("send", [&](CallClientConfig* c) {
    c.transport.rates.start_rate = StartRate;
  });
let route = s.CreateRoutes(
      client, {net.node()}, s.CreateClient("return", CallClientConfig()),
      {s.CreateSimulationNode(NetworkSimulationConfig())});

  // Create a config without packet feedback.
  VideoStreamConfig video_config;
  video_config.stream.packet_feedback = false;
  s.CreateVideoStream(route.forward(), video_config);

  s.RunFor(TimeDelta::Seconds(20));
  // Bandwith does not backoff because network is normal.
  EXPECT_GE(client.target_rate().kbps(), 500);

  // Update the network to create high loss ratio
  net.UpdateConfig([](NetworkSimulationConfig* c) { c.loss_rate = 0.15; });
  s.RunFor(TimeDelta::Seconds(20));

  // Bandwidth decreases thanks to loss based bwe v0.
  EXPECT_LE(client.target_rate().kbps(), 300);
}

class GoogCcRttTest : public ::testing::TestWithParam<bool> {
 protected:
fn Config(feedback_only: bool) -> GoogCcFactoryConfig {
    GoogCcFactoryConfig config;
    config.feedback_only = feedback_only;
    return config;
  }
};

TEST_P(GoogCcRttTest, CalculatesRttFromTransporFeedback) {
  GoogCcFactoryConfig config(Config(/*feedback_only=*/GetParam()));
  if (!GetParam()) {
    // TODO(diepbp): understand the usage difference between
    // UpdatePropagationRtt and UpdateRtt
    GTEST_SKIP() << "This test should run only if "
                    "feedback_only is enabled";
  }
  NetworkControllerTestFixture fixture(std::move(config));
  std::unique_ptr<NetworkControllerInterface> controller =
      fixture.CreateController();
  let current_time: Timestamp = Timestamp::Millis(123);
  let one_way_delay: TimeDelta = TimeDelta::Millis(10);
  Option<TimeDelta> rtt = None;

  let feedback: TransportPacketsFeedback = CreateTransportPacketsFeedback(
      /*per_packet_network_delay=*/TimeDelta::Millis(50), one_way_delay,
      /*send_time=*/current_time);
  let update: NetworkControlUpdate =
      controller.OnTransportPacketsFeedback(feedback);
  current_time += TimeDelta::Millis(50);
  update = controller.OnProcessInterval({.at_time = current_time});
  if (update.target_rate) {
    rtt = update.target_rate.network_estimate.round_trip_time;
  }
  assert!(rtt.is_some());
  assert_eq!(rtt.ms(), 2 * one_way_delay.ms());
}

INSTANTIATE_TEST_SUITE_P(GoogCcRttTests, GoogCcRttTest, ::testing::Bool());

}  // namespace test
}  // namespace webrtc
