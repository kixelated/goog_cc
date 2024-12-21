/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use super::{
    transport::*,
    units::{DataRate, TimeDelta},
};

pub trait TargetTransferRateObserver {
    // Called to indicate target transfer rate as well as giving information about
    // the current estimate of network parameters.
    fn OnTargetTransferRate(target_transfer_rate: TargetTransferRate);
    // Called to provide updates to the expected target rate in case it changes
    // before the first call to OnTargetTransferRate.
    fn OnStartRateUpdate(data_rate: DataRate);
}

// Configuration sent to factory create function. The parameters here are
// optional to use for a network controller implementation.
#[derive(Default)]
pub struct NetworkControllerConfig {
    // The initial constraints to start with, these can be changed at any later
    // time by calls to OnTargetRateConstraints. Note that the starting rate
    // has to be set initially to provide a starting state for the network
    // controller, even though the field is marked as optional.
    pub constraints: TargetRateConstraints,
    // Initial stream specific configuration, these are changed at any later time
    // by calls to OnStreamsConfig.
    pub stream_based_config: StreamsConfig,
}

// NetworkControllerInterface is implemented by network controllers. A network
// controller is a class that uses information about network state and traffic
// to estimate network parameters such as round trip time and bandwidth. Network
// controllers does not guarantee thread safety, the interface must be used in a
// non-concurrent fashion.
pub trait NetworkControllerInterface {
    // Called when network availabilty changes.
    fn OnNetworkAvailability(&mut self, event: NetworkAvailability) -> NetworkControlUpdate;
    // Called when the receiving or sending endpoint changes address.
    fn OnNetworkRouteChange(&mut self, event: NetworkRouteChange) -> NetworkControlUpdate;
    // Called periodically with a periodicy as specified by
    // NetworkControllerFactoryInterface::GetProcessInterval.
    fn OnProcessInterval(&mut self, event: ProcessInterval) -> NetworkControlUpdate;
    // Called when remotely calculated bitrate is received.
    fn OnRemoteBitrateReport(&mut self, event: RemoteBitrateReport) -> NetworkControlUpdate;
    // Called round trip time has been calculated by protocol specific mechanisms.
    fn OnRoundTripTimeUpdate(&mut self, event: RoundTripTimeUpdate) -> NetworkControlUpdate;
    // Called when a packet is sent on the network.
    fn OnSentPacket(&mut self, event: SentPacket) -> NetworkControlUpdate;
    // Called when a packet is received from the remote client.
    fn OnReceivedPacket(&mut self, event: ReceivedPacket) -> NetworkControlUpdate;
    // Called when the stream specific configuration has been updated.
    fn OnStreamsConfig(&mut self, event: StreamsConfig) -> NetworkControlUpdate;
    // Called when target transfer rate constraints has been changed.
    fn OnTargetRateConstraints(&mut self, event: TargetRateConstraints) -> NetworkControlUpdate;
    // Called when a protocol specific calculation of packet loss has been made.
    fn OnTransportLossReport(&mut self, event: TransportLossReport) -> NetworkControlUpdate;
    // Called with per packet feedback regarding receive time.
    fn OnTransportPacketsFeedback(
        &mut self,
        event: TransportPacketsFeedback,
    ) -> NetworkControlUpdate;
    // Called with network state estimate updates.
    fn OnNetworkStateEstimate(&mut self, event: NetworkStateEstimate) -> NetworkControlUpdate;
}

// NetworkControllerFactoryInterface is an interface for creating a network
// controller.
pub trait NetworkControllerFactoryInterface {
    // Used to create a new network controller, requires an observer to be
    // provided to handle callbacks.
    fn Create(config: NetworkControllerConfig) -> impl NetworkControllerInterface;
    // Returns the interval by which the network controller expects
    // OnProcessInterval calls.
    fn GetProcessInterval() -> TimeDelta;
}

// Under development, subject to change without notice.
pub trait NetworkStateEstimator {
    // Gets the current best estimate according to the estimator.
    fn GetCurrentEstimate(&self) -> Option<NetworkStateEstimate>;
    // Called with per packet feedback regarding receive time.
    // Used when the NetworkStateEstimator runs in the sending endpoint.
    fn OnTransportPacketsFeedback(&mut self, event: &TransportPacketsFeedback);
    // Called with per packet feedback regarding receive time.
    // Used when the NetworkStateEstimator runs in the receiving endpoint.
    fn OnReceivedPacket(&mut self, event: &PacketResult);
    // Called when the receiving or sending endpoint changes address.
    fn OnRouteChange(&mut self, event: &NetworkRouteChange);
}

pub trait NetworkStateEstimatorFactory {
    fn Create() -> impl NetworkStateEstimator;
}
