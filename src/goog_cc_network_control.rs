/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::VecDeque;

// lul this struct uses everything
use crate::{
    api::{transport::*, units::*, *},
    *,
};

#[derive(Default)]
pub struct GoogCcConfig {
    network_state_estimator: Option<Box<dyn NetworkStateEstimator>>,
    network_state_predictor: Option<Box<dyn NetworkStatePredictor>>,
    feedback_only: bool,
}

pub struct GoogCcNetworkController {
    packet_feedback_only: bool,
    safe_reset_on_route_change: bool,
    safe_reset_acknowledged_rate: bool,
    use_min_allocatable_as_lower_bound: bool,
    ignore_probes_lower_than_network_estimate: bool,
    limit_probes_lower_than_throughput_estimate: bool,
    rate_control_settings: bool,
    pace_at_max_of_bwe_and_lower_link_capacity: bool,
    limit_pacingfactor_by_upper_link_capacity_estimate: bool,

    probe_controller: ProbeController,
    congestion_window_pushback_controller: CongestionWindowPushbackController,

    bandwidth_estimation: SendSideBandwidthEstimation,
    alr_detector: AlrDetector,
    probe_bitrate_estimator: ProbeBitrateEstimator,
    network_estimator: Box<dyn NetworkStateEstimator>,
    network_state_predictor: Box<dyn NetworkStatePredictor>,
    delay_based_bwe: DelayBasedBwe,
    acknowledged_bitrate_estimator: Box<dyn AcknowledgedBitrateEstimatorInterface>,

    initial_config: Option<NetworkControllerConfig>,

    min_target_rate: DataRate,
    min_data_rate: DataRate,
    max_data_rate: DataRate, // = DataRate::PlusInfinity();
    starting_rate: Option<DataRate>,

    first_packet_sent: bool,

    estimate: Option<NetworkStateEstimate>,

    next_loss_update: Timestamp, // = Timestamp::MinusInfinity();
    lost_packets_since_last_loss_update: isize,
    expected_packets_since_last_loss_update: isize,

    feedback_max_rtts: VecDeque<i64>,

    last_loss_based_target_rate: DataRate,
    last_pushback_target_rate: DataRate,
    last_stable_target_rate: DataRate,
    last_loss_base_state: LossBasedState,

    last_estimated_fraction_loss: Option<u8>,
    last_estimated_round_trip_time: TimeDelta, //= TimeDelta::PlusInfinity();

    pacing_factor: f64,
    min_total_allocated_bitrate: DataRate,
    max_padding_rate: DataRate,

    previously_in_alr: bool,

    current_data_window: Option<DataSize>,
}

impl GoogCcNetworkController {
    pub fn new(config: NetworkControllerConfig, goog_cc_config: GoogCcConfig) -> Self {
        todo!()
    }

    pub fn GetNetworkState(&self, at_time: Timestamp) -> NetworkControlUpdate {
        todo!()
    }

    fn ResetConstraints(
        &mut self,
        new_constraints: TargetRateConstraints,
    ) -> Vec<ProbeClusterConfig> {
        todo!()
    }

    fn ClampConstraints(&mut self) {
        todo!();
    }
    fn MaybeTriggerOnNetworkChanged(update: &NetworkControlUpdate, at_time: Timestamp) {
        todo!()
    }

    fn UpdateCongestionWindowlen(&mut self) {}
    fn GetPacingRates(&self, at_time: Timestamp) -> PacerConfig {
        todo!()
    }
    fn SetNetworkStateEstimate(estimate: Option<NetworkStateEstimate>) {
        todo!()
    }
}

impl NetworkControllerInterface for GoogCcNetworkController {
    fn OnNetworkAvailability(&mut self, event: NetworkAvailability) -> NetworkControlUpdate {
        todo!()
    }

    fn OnNetworkRouteChange(&mut self, event: NetworkRouteChange) -> NetworkControlUpdate {
        todo!()
    }

    fn OnProcessInterval(&mut self, event: ProcessInterval) -> NetworkControlUpdate {
        todo!()
    }

    fn OnRemoteBitrateReport(&mut self, event: RemoteBitrateReport) -> NetworkControlUpdate {
        todo!()
    }

    fn OnRoundTripTimeUpdate(&mut self, event: RoundTripTimeUpdate) -> NetworkControlUpdate {
        todo!()
    }

    fn OnSentPacket(&mut self, event: SentPacket) -> NetworkControlUpdate {
        todo!()
    }

    fn OnReceivedPacket(&mut self, event: ReceivedPacket) -> NetworkControlUpdate {
        todo!()
    }

    fn OnStreamsConfig(&mut self, event: StreamsConfig) -> NetworkControlUpdate {
        todo!()
    }

    fn OnTargetRateConstraints(&mut self, event: TargetRateConstraints) -> NetworkControlUpdate {
        todo!()
    }

    fn OnTransportLossReport(&mut self, event: TransportLossReport) -> NetworkControlUpdate {
        todo!()
    }

    fn OnTransportPacketsFeedback(
        &mut self,
        event: TransportPacketsFeedback,
    ) -> NetworkControlUpdate {
        todo!()
    }

    fn OnNetworkStateEstimate(&mut self, event: NetworkStateEstimate) -> NetworkControlUpdate {
        todo!()
    }
}
