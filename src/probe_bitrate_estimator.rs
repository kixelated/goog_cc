/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::collections::HashMap;

use crate::api::{
    transport::{PacedPacketInfo, PacketResult},
    units::{DataRate, DataSize, TimeDelta, Timestamp},
};

struct AggregatedCluster {
    pub num_probes: i64,
    pub first_send: Timestamp,
    pub last_send: Timestamp,
    pub first_receive: Timestamp,
    pub last_receive: Timestamp,
    pub size_last_send: DataSize,
    pub size_first_receive: DataSize,
    pub size_total: DataSize,
}

impl Default for AggregatedCluster {
    fn default() -> Self {
        Self {
            num_probes: 0,
            first_send: Timestamp::PlusInfinity(),
            last_send: Timestamp::MinusInfinity(),
            first_receive: Timestamp::PlusInfinity(),
            last_receive: Timestamp::MinusInfinity(),
            size_last_send: DataSize::Zero(),
            size_first_receive: DataSize::Zero(),
            size_total: DataSize::Zero(),
        }
    }
}

#[derive(Default)]
pub struct ProbeBitrateEstimator {
    clusters: HashMap<i64, AggregatedCluster>,
    estimated_data_rate: Option<DataRate>,
}

impl ProbeBitrateEstimator {
    // The minumum number of probes we need to receive feedback about in percent
    // in order to have a valid estimate.
    const MinReceivedProbesRatio: f64 = 0.80;

    // The minumum number of bytes we need to receive feedback about in percent
    // in order to have a valid estimate.
    const MinReceivedBytesRatio: f64 = 0.80;

    // The maximum |receive rate| / |send rate| ratio for a valid estimate.
    const MaxValidRatio: f64 = 2.0;

    // The minimum |receive rate| / |send rate| ratio assuming that the link is
    // not saturated, i.e. we assume that we will receive at least
    // MinRatioForUnsaturatedLink * |send rate| if |send rate| is less than the
    // link capacity.
    const MinRatioForUnsaturatedLink: f32 = 0.9;

    // The target utilization of the link. If we know true link capacity
    // we'd like to send at 95% of that rate.
    const TargetUtilizationFraction: f32 = 0.95;

    // The maximum time period over which the cluster history is retained.
    // This is also the maximum time period beyond which a probing burst is not
    // expected to last.
    const MaxClusterHistory: TimeDelta = TimeDelta::Seconds(1);

    // The maximum time interval between first and the last probe on a cluster
    // on the sender side as well as the receive side.
    const MaxProbeInterval: TimeDelta = TimeDelta::Seconds(1);

    // Should be called for every probe packet we receive feedback about.
    // Returns the estimated bitrate if the probe completes a valid cluster.
    pub fn HandleProbeAndEstimateBitrate(
        &mut self,
        packet_feedback: &PacketResult,
    ) -> Option<DataRate> {
        let cluster_id: i64 = packet_feedback.sent_packet.pacing_info.probe_cluster_id;
        assert_ne!(cluster_id, PacedPacketInfo::NotAProbe);

        self.EraseOldClusters(packet_feedback.receive_time);

        let cluster: &mut AggregatedCluster = self.clusters.entry(cluster_id).or_default();

        if packet_feedback.sent_packet.send_time < cluster.first_send {
            cluster.first_send = packet_feedback.sent_packet.send_time;
        }
        if packet_feedback.sent_packet.send_time > cluster.last_send {
            cluster.last_send = packet_feedback.sent_packet.send_time;
            cluster.size_last_send = packet_feedback.sent_packet.size;
        }
        if packet_feedback.receive_time < cluster.first_receive {
            cluster.first_receive = packet_feedback.receive_time;
            cluster.size_first_receive = packet_feedback.sent_packet.size;
        }
        if packet_feedback.receive_time > cluster.last_receive {
            cluster.last_receive = packet_feedback.receive_time;
        }
        cluster.size_total += packet_feedback.sent_packet.size;
        cluster.num_probes += 1;

        assert!(
            packet_feedback
                .sent_packet
                .pacing_info
                .probe_cluster_min_probes
                > 0
        );
        assert!(
            packet_feedback
                .sent_packet
                .pacing_info
                .probe_cluster_min_bytes
                > 0
        );

        let min_probes: i64 = (packet_feedback
            .sent_packet
            .pacing_info
            .probe_cluster_min_probes as f64
            * Self::MinReceivedProbesRatio) as i64;
        let min_size: DataSize = DataSize::Bytes(
            packet_feedback
                .sent_packet
                .pacing_info
                .probe_cluster_min_bytes as _,
        ) * Self::MinReceivedBytesRatio;
        if cluster.num_probes < min_probes || cluster.size_total < min_size {
            return None;
        }

        let send_interval: TimeDelta = cluster.last_send - cluster.first_send;
        let receive_interval: TimeDelta = cluster.last_receive - cluster.first_receive;

        if send_interval <= TimeDelta::Zero()
            || send_interval > Self::MaxProbeInterval
            || receive_interval <= TimeDelta::Zero()
            || receive_interval > Self::MaxProbeInterval
        {
            tracing::info!("Probing unsuccessful, invalid send/receive interval [cluster id: {}] [send interval: {:?}] [receive interval: {:?}]",
                      cluster_id, send_interval, receive_interval);
            return None;
        }
        // Since the `send_interval` does not include the time it takes to actually
        // send the last packet the size of the last sent packet should not be
        // included when calculating the send bitrate.
        assert!(cluster.size_total > cluster.size_last_send);
        let send_size: DataSize = cluster.size_total - cluster.size_last_send;
        let send_rate: DataRate = send_size / send_interval;

        // Since the `receive_interval` does not include the time it takes to
        // actually receive the first packet the size of the first received packet
        // should not be included when calculating the receive bitrate.
        assert!(cluster.size_total > cluster.size_first_receive);
        let receive_size: DataSize = cluster.size_total - cluster.size_first_receive;
        let receive_rate: DataRate = receive_size / receive_interval;

        let ratio: f64 = receive_rate / send_rate;
        if ratio > Self::MaxValidRatio {
            tracing::info!("Probing unsuccessful, receive/send ratio too high [cluster id: {}] [send: {:?}/{:?} = {:?}] [receive: {:?}/{:?} = {:?}] [ratio: {:?} > {:?}]",
                      cluster_id, send_size, send_interval, send_rate, receive_size, receive_interval, receive_rate, ratio, Self::MaxValidRatio);
            return None;
        }

        tracing::info!("Probing successful [cluster id: {}] [send: {:?}/{:?} = {:?}] [receive: {:?}/{:?} = {:?}]",
                    cluster_id, send_size, send_interval, send_rate, receive_size, receive_interval, receive_rate);

        let mut res: DataRate = std::cmp::min(send_rate, receive_rate);
        // If we're receiving at significantly lower bitrate than we were sending at,
        // it suggests that we've found the true capacity of the link. In this case,
        // set the target bitrate slightly lower to not immediately overuse.
        if receive_rate < Self::MinRatioForUnsaturatedLink * send_rate {
            assert!(send_rate > receive_rate);
            res = Self::TargetUtilizationFraction * receive_rate;
        }
        self.estimated_data_rate = Some(res);
        self.estimated_data_rate
    }

    pub fn FetchAndResetLastEstimatedBitrate(&mut self) -> Option<DataRate> {
        let estimated_data_rate: Option<DataRate> = self.estimated_data_rate;
        self.estimated_data_rate.take();
        estimated_data_rate
    }

    // Erases old cluster data that was seen before `timestamp`.
    fn EraseOldClusters(&mut self, timestamp: Timestamp) {
        self.clusters
            .retain(|_, cluster| cluster.last_receive + Self::MaxClusterHistory >= timestamp);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    //use crate::api::transport::PacedPacketInfo;

    const DefaultMinProbes: i64 = 5;
    const DefaultMinBytes: i64 = 5000;
    const TargetUtilizationFraction: f64 = 0.95;

    #[derive(Default)]
    struct TestProbeBitrateEstimator {
        measured_data_rate: Option<DataRate>,
        probe_bitrate_estimator: ProbeBitrateEstimator,
    }

    impl TestProbeBitrateEstimator {
        // TODO(philipel): Use PacedPacketInfo when ProbeBitrateEstimator is rewritten
        //                 to use that information.
        fn AddPacketFeedback(
            &mut self,
            probe_cluster_id: i64,
            size_bytes: usize,
            send_time_ms: i64,
            arrival_time_ms: i64,
            min_probes: i64,
            min_bytes: i64,
        ) {
            const ReferenceTime: Timestamp = Timestamp::Seconds(1000);
            let mut feedback: PacketResult = PacketResult::default();
            feedback.sent_packet.send_time = ReferenceTime + TimeDelta::Millis(send_time_ms);
            feedback.sent_packet.size = DataSize::Bytes(size_bytes as _);
            feedback.sent_packet.pacing_info =
                PacedPacketInfo::new(probe_cluster_id, min_probes, min_bytes);
            feedback.receive_time = ReferenceTime + TimeDelta::Millis(arrival_time_ms);
            self.measured_data_rate = self
                .probe_bitrate_estimator
                .HandleProbeAndEstimateBitrate(&feedback);
        }
    }

    #[test]
    fn OneCluster() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 30, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 30, 40, DefaultMinProbes, DefaultMinBytes);

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            800000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn OneClusterTooFewProbes() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 2000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 2000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 2000, 20, 30, DefaultMinProbes, DefaultMinBytes);

        assert!(test.measured_data_rate.is_none());
    }

    #[test]
    fn OneClusterTooFewBytes() {
        let mut test = TestProbeBitrateEstimator::default();
        const MinBytes: i64 = 6000;
        test.AddPacketFeedback(0, 800, 0, 10, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 800, 10, 20, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 800, 20, 30, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 800, 30, 40, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 800, 40, 50, DefaultMinProbes, MinBytes);

        assert!(test.measured_data_rate.is_none());
    }

    #[test]
    fn SmallCluster() {
        let mut test = TestProbeBitrateEstimator::default();
        const MinBytes: i64 = 1000;
        test.AddPacketFeedback(0, 150, 0, 10, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 150, 10, 20, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 150, 20, 30, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 150, 30, 40, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 150, 40, 50, DefaultMinProbes, MinBytes);
        test.AddPacketFeedback(0, 150, 50, 60, DefaultMinProbes, MinBytes);
        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            120000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn LargeCluster() {
        let mut test = TestProbeBitrateEstimator::default();
        const MinProbes: i64 = 30;
        const MinBytes: i64 = 312500;

        let mut send_time: i64 = 0;
        let mut receive_time: i64 = 5;
        for i in 0..25 {
            test.AddPacketFeedback(0, 12500, send_time, receive_time, MinProbes, MinBytes);
            send_time += 1;
            receive_time += 1;
        }
        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            100000000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn FastReceive() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 15, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 30, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 35, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 30, 40, DefaultMinProbes, DefaultMinBytes);

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            800000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn TooFastReceive() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 19, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 22, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 25, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 40, 27, DefaultMinProbes, DefaultMinBytes);

        assert!(test.measured_data_rate.is_none());
    }

    #[test]
    fn SlowReceive() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 40, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 70, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 30, 85, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 800 kbps, expected receive rate = 320 kbps.

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            TargetUtilizationFraction * 320000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn BurstReceive() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 50, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 50, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 50, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 40, 50, DefaultMinProbes, DefaultMinBytes);

        assert!(test.measured_data_rate.is_none());
    }

    #[test]
    fn MultipleClusters() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 30, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 40, 60, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 600 kbps, expected receive rate = 480 kbps.
        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            TargetUtilizationFraction * 480000.0,
            epsilon = 10.0
        );

        test.AddPacketFeedback(0, 1000, 50, 60, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 640 kbps, expected receive rate = 640 kbps.
        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            640000.0,
            epsilon = 10.0
        );

        test.AddPacketFeedback(1, 1000, 60, 70, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(1, 1000, 65, 77, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(1, 1000, 70, 84, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(1, 1000, 75, 90, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 1600 kbps, expected receive rate = 1200 kbps.

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            TargetUtilizationFraction * 1200000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn IgnoreOldClusters() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 30, DefaultMinProbes, DefaultMinBytes);

        test.AddPacketFeedback(1, 1000, 60, 70, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(1, 1000, 65, 77, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(1, 1000, 70, 84, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(1, 1000, 75, 90, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 1600 kbps, expected receive rate = 1200 kbps.

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            TargetUtilizationFraction * 1200000.0,
            epsilon = 10.0
        );

        // Coming in 6s later
        test.AddPacketFeedback(
            0,
            1000,
            40 + 6000,
            60 + 6000,
            DefaultMinProbes,
            DefaultMinBytes,
        );

        assert!(test.measured_data_rate.is_none());
    }

    #[test]
    fn IgnoreSizeLastSendPacket() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 30, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 30, 40, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1500, 40, 50, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 800 kbps, expected receive rate = 900 kbps.

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            800000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn IgnoreSizeFirstReceivePacket() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1500, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 30, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 30, 40, DefaultMinProbes, DefaultMinBytes);
        // Expected send rate = 933 kbps, expected receive rate = 800 kbps.

        assert_relative_eq!(
            test.measured_data_rate.unwrap().bps_float(),
            TargetUtilizationFraction * 800000.0,
            epsilon = 10.0
        );
    }

    #[test]
    fn NoLastEstimatedBitrateBps() {
        let mut test = TestProbeBitrateEstimator::default();
        assert!(test
            .probe_bitrate_estimator
            .FetchAndResetLastEstimatedBitrate()
            .is_none());
    }

    #[test]
    fn FetchLastEstimatedBitrateBps() {
        let mut test = TestProbeBitrateEstimator::default();
        test.AddPacketFeedback(0, 1000, 0, 10, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 10, 20, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 20, 30, DefaultMinProbes, DefaultMinBytes);
        test.AddPacketFeedback(0, 1000, 30, 40, DefaultMinProbes, DefaultMinBytes);

        let estimated_bitrate = test
            .probe_bitrate_estimator
            .FetchAndResetLastEstimatedBitrate();
        assert!(estimated_bitrate.is_some());
        assert_relative_eq!(
            estimated_bitrate.unwrap().bps_float(),
            800000.0,
            epsilon = 10.0
        );
        assert!(test
            .probe_bitrate_estimator
            .FetchAndResetLastEstimatedBitrate()
            .is_none());
    }
} // namespace webrtc
