/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_GOOG_CC_NETWORK_CONTROL_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_GOOG_CC_NETWORK_CONTROL_H_

#include <stdint.h>

#include <deque>
#include <memory>
#include <optional>
#include <vector>

#include "api/environment/environment.h"
#include "api/network_state_predictor.h"
#include "api/transport/network_control.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/acknowledged_bitrate_estimator_interface.h"
#include "modules/congestion_controller/goog_cc/alr_detector.h"
#include "modules/congestion_controller/goog_cc/congestion_window_pushback_controller.h"
#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"
#include "modules/congestion_controller/goog_cc/loss_based_bwe_v2.h"
#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"
#include "modules/congestion_controller/goog_cc/probe_controller.h"
#include "modules/congestion_controller/goog_cc/send_side_bandwidth_estimation.h"
#include "rtc_base/experiments/field_trial_parser.h"
#include "rtc_base/experiments/rate_control_settings.h"


struct GoogCcConfig {
  std::unique_ptr<NetworkStateEstimator> network_state_estimator = nullptr;
  std::unique_ptr<NetworkStatePredictor> network_state_predictor = nullptr;
  bool feedback_only = false;
};

impl NetworkControllerInterface for GoogCcNetworkController {

}

pub struct GoogCcNetworkController {
 public:
  GoogCcNetworkController(NetworkControllerConfig config,
                          GoogCcConfig goog_cc_config);

  GoogCcNetworkController() = delete;
  GoogCcNetworkController(const GoogCcNetworkController&) = delete;
  GoogCcNetworkController& operator=(const GoogCcNetworkController&) = delete;

  ~GoogCcNetworkController() override;

  // NetworkControllerInterface
  NetworkControlUpdate OnNetworkAvailability(NetworkAvailability msg) override;
  NetworkControlUpdate OnNetworkRouteChange(NetworkRouteChange msg) override;
  NetworkControlUpdate OnProcessInterval(ProcessInterval msg) override;
  NetworkControlUpdate OnRemoteBitrateReport(RemoteBitrateReport msg) override;
  NetworkControlUpdate OnRoundTripTimeUpdate(RoundTripTimeUpdate msg) override;
  NetworkControlUpdate OnSentPacket(SentPacket msg) override;
  NetworkControlUpdate OnReceivedPacket(ReceivedPacket msg) override;
  NetworkControlUpdate OnStreamsConfig(StreamsConfig msg) override;
  NetworkControlUpdate OnTargetRateConstraints(
      TargetRateConstraints msg) override;
  NetworkControlUpdate OnTransportLossReport(TransportLossReport msg) override;
  NetworkControlUpdate OnTransportPacketsFeedback(
      TransportPacketsFeedback msg) override;
  NetworkControlUpdate OnNetworkStateEstimate(
      NetworkStateEstimate msg) override;

  NetworkControlUpdate GetNetworkState(Timestamp at_time) const;

 private:
  friend class GoogCcStatePrinter;
  Vec<ProbeClusterConfig> ResetConstraints(
      TargetRateConstraints new_constraints);
  void ClampConstraints();
  void MaybeTriggerOnNetworkChanged(NetworkControlUpdate* update,
                                    Timestamp at_time);
  void UpdateCongestionWindowlen();
  PacerConfig GetPacingRates(Timestamp at_time) const;
  fn SetNetworkStateEstimate(Option<NetworkStateEstimate> estimate) {
  todo!();
}

  const Environment self.env;
  const bool self.packet_feedback_only;
  safe_reset_on_route_change: FieldTrialFlag,
  safe_reset_acknowledged_rate: FieldTrialFlag,
  const bool self.use_min_allocatable_as_lower_bound;
  const bool self.ignore_probes_lower_than_network_estimate;
  const bool self.limit_probes_lower_than_throughput_estimate;
  const RateControlSettings self.rate_control_settings;
  const bool self.pace_at_max_of_bwe_and_lower_link_capacity;
  const bool self.limit_pacingfactor_by_upper_link_capacity_estimate;

  const std::unique_ptr<ProbeController> self.probe_controller;
  const std::unique_ptr<CongestionWindowPushbackController>
      self.congestion_window_pushback_controller;

  bandwidth_estimation: std::unique_ptr<SendSideBandwidthEstimation>,
  alr_detector: std::unique_ptr<AlrDetector>,
  probe_bitrate_estimator: std::unique_ptr<ProbeBitrateEstimator>,
  network_estimator: std::unique_ptr<NetworkStateEstimator>,
  network_state_predictor: std::unique_ptr<NetworkStatePredictor>,
  delay_based_bwe: std::unique_ptr<DelayBasedBwe>,
  std::unique_ptr<AcknowledgedBitrateEstimatorInterface>
      self.acknowledged_bitrate_estimator;

  initial_config: Option<NetworkControllerConfig>,

  DataRate self.min_target_rate = DataRate::Zero();
  DataRate self.min_data_rate = DataRate::Zero();
  DataRate self.max_data_rate = DataRate::PlusInfinity();
  starting_rate: Option<DataRate>,

  bool self.first_packet_sent = false;

  estimate: Option<NetworkStateEstimate>,

  Timestamp self.next_loss_update = Timestamp::MinusInfinity();
  int self.lost_packets_since_last_loss_update = 0;
  int self.expected_packets_since_last_loss_update = 0;

  feedback_max_rtts: VecDeque<i64>,

  last_loss_based_target_rate: DataRate,
  last_pushback_target_rate: DataRate,
  last_stable_target_rate: DataRate,
  last_loss_base_state: LossBasedState,

  Option<uint8_t> self.last_estimated_fraction_loss = 0;
  TimeDelta self.last_estimated_round_trip_time = TimeDelta::PlusInfinity();

  pacing_factor: f64,
  min_total_allocated_bitrate: DataRate,
  max_padding_rate: DataRate,

  bool self.previously_in_alr = false;

  current_data_window: Option<DataSize>,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_GOOG_CC_NETWORK_CONTROL_H_
