/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use crate::{
    api::{
        transport::{BandwidthUsage, PacketResult, TransportPacketsFeedback},
        units::{DataRate, DataSize, TimeDelta, Timestamp},
    },
    remote_bitrate_estimator::{AimdRateControl, RateControlInput},
    DelayIncreaseDetectorInterface, FieldTrials, InterArrivalDelta, TrendlineEstimator,
};

// WebRTC-Bwe-SeparateAudioPackets
#[derive(Clone, Debug)]
pub struct BweSeparateAudioPacketsSettings {
    pub enabled: bool,
    pub packet_threshold: i64,
    pub time_threshold: TimeDelta,
}

impl Default for BweSeparateAudioPacketsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            packet_threshold: 10,
            time_threshold: TimeDelta::Seconds(1),
        }
    }
}

pub struct DelayBasedBweResult {
    pub updated: bool,
    pub probe: bool,
    pub target_bitrate: DataRate,
    pub recovered_from_overuse: bool,
    pub delay_detector_state: BandwidthUsage,
}

impl Default for DelayBasedBweResult {
    fn default() -> Self {
        Self {
            updated: false,
            probe: false,
            target_bitrate: DataRate::Zero(),
            recovered_from_overuse: false,
            delay_detector_state: BandwidthUsage::Normal,
        }
    }
}

enum DelayDetector {
    Video,
    Audio,
}

pub struct DelayBasedBwe {
    // Alternatively, run two separate overuse detectors for audio and video,
    // and fall back to the audio one if we haven't seen a video packet in a
    // while.
    separate_audio: BweSeparateAudioPacketsSettings,
    audio_packets_since_last_video: i64,
    last_video_packet_recv_time: Timestamp,

    // Unused?
    //video_inter_arrival: InterArrival,
    video_inter_arrival_delta: InterArrivalDelta,
    video_delay_detector: TrendlineEstimator,
    // Unused?
    //audio_inter_arrival: InterArrival,
    audio_inter_arrival_delta: InterArrivalDelta,
    audio_delay_detector: TrendlineEstimator,
    active_delay_detector_type: DelayDetector,

    last_seen_packet: Timestamp,
    uma_recorded: bool,
    rate_control: AimdRateControl,
    prev_bitrate: DataRate,
    prev_state: BandwidthUsage,
}

impl DelayBasedBwe {
    const StreamTimeOut: TimeDelta = TimeDelta::Seconds(2);
    const SendTimeGroupLength: TimeDelta = TimeDelta::Millis(5);

    // This ssrc is used to fulfill the current API but will be removed
    // after the API has been changed.
    const FixedSsrc: u32 = 0;

    pub fn new(field_trials: &FieldTrials) -> Self {
        tracing::info!("Initialized DelayBasedBwe with separate audio overuse detection: {:?}", field_trials.separate_audio_packets);
        Self {
            separate_audio: field_trials.separate_audio_packets.clone(),
            audio_packets_since_last_video: 0,
            last_video_packet_recv_time: Timestamp::MinusInfinity(),
            video_delay_detector: TrendlineEstimator::new(
                field_trials.trendline_estimator_settings.clone(),
            ),
            audio_delay_detector: TrendlineEstimator::new(
                field_trials.trendline_estimator_settings.clone(),
            ),
            active_delay_detector_type: DelayDetector::Video,
            last_seen_packet: Timestamp::MinusInfinity(),
            uma_recorded: false,
            rate_control: AimdRateControl::new(field_trials, true), // send_side
            prev_bitrate: DataRate::Zero(),
            prev_state: BandwidthUsage::Normal,

            // Note: Not initialized in C++ but this avoids needing an Option
            audio_inter_arrival_delta: InterArrivalDelta::new(Self::SendTimeGroupLength),
            video_inter_arrival_delta: InterArrivalDelta::new(Self::SendTimeGroupLength),
        }
    }

    fn active_delay_detector_state(&self) -> BandwidthUsage {
        match self.active_delay_detector_type {
            DelayDetector::Audio => self.audio_delay_detector.state(),
            DelayDetector::Video => self.video_delay_detector.state(),
        }
    }

    pub fn IncomingPacketFeedbackVector(
        &mut self,
        msg: &TransportPacketsFeedback,
        acked_bitrate: Option<DataRate>,
        probe_bitrate: Option<DataRate>,
        //network_estimate: Option<NetworkStateEstimate>,
        in_alr: bool,
    ) -> DelayBasedBweResult {
        let packet_feedback_vector = msg.SortedByReceiveTime();
        // TODO(holmer): An empty feedback vector here likely means that
        // all acks were too late and that the send time history had
        // timed out. We should reduce the rate when this occurs.
        if packet_feedback_vector.is_empty() {
            tracing::warn!("Very late feedback received.");
            return DelayBasedBweResult::default();
        }

        if !self.uma_recorded {
            self.uma_recorded = true;
        }
        let mut delayed_feedback: bool = true;
        let mut recovered_from_overuse: bool = false;
        let mut prev_detector_state: BandwidthUsage = self.active_delay_detector_state();
        for packet_feedback in &packet_feedback_vector {
            delayed_feedback = false;
            self.IncomingPacketFeedback(packet_feedback, msg.feedback_time);
            if prev_detector_state == BandwidthUsage::Underusing
                && self.active_delay_detector_state() == BandwidthUsage::Normal
            {
                recovered_from_overuse = true;
            }
            prev_detector_state = self.active_delay_detector_state();
        }

        if delayed_feedback {
            // TODO(bugs.webrtc.org/10125): Design a better mechanism to safe-guard
            // against building very large network queues.
            return DelayBasedBweResult::default();
        }
        self.rate_control.SetInApplicationLimitedRegion(in_alr);
        //self.rate_control.SetNetworkStateEstimate(network_estimate);
        self.MaybeUpdateEstimate(
            acked_bitrate,
            probe_bitrate,
            //network_estimate,
            recovered_from_overuse,
            in_alr,
            msg.feedback_time,
        )
    }

    pub fn OnRttUpdate(&mut self, avg_rtt: TimeDelta) {
        self.rate_control.SetRtt(avg_rtt);
    }
    pub fn LatestEstimate(&self, ssrcs: &mut Vec<u32>, bitrate: &mut DataRate) -> bool {
        // Currently accessed from both the process thread (see
        // ModuleRtpRtcpImpl::Process()) and the configuration thread (see
        // Call::GetStats()). Should in the future only be accessed from a single
        // thread.
        if !self.rate_control.ValidEstimate() {
            return false;
        }

        *ssrcs = vec![Self::FixedSsrc];
        *bitrate = self.rate_control.LatestEstimate();
        true
    }

    pub fn SetStartBitrate(&mut self, start_bitrate: DataRate) {
        tracing::info!("BWE Setting start bitrate to: {:?}", start_bitrate);
        self.rate_control.SetStartBitrate(start_bitrate);
    }
    pub fn SetMinBitrate(&mut self, min_bitrate: DataRate) {
        // Called from both the configuration thread and the network thread. Shouldn't
        // be called from the network thread in the future.
        self.rate_control.SetMinBitrate(min_bitrate);
    }

    pub fn GetExpectedBwePeriod(&self) -> TimeDelta {
        self.rate_control.GetExpectedBandwidthPeriod()
    }
    pub fn TriggerOveruse(
        &mut self,
        at_time: Timestamp,
        link_capacity: Option<DataRate>,
    ) -> DataRate {
        let input = RateControlInput::new(BandwidthUsage::Overusing, link_capacity);
        self.rate_control.Update(input, at_time)
    }
    pub fn last_estimate(&self) -> DataRate {
        self.prev_bitrate
    }
    pub fn last_state(&self) -> BandwidthUsage {
        self.prev_state
    }

    fn IncomingPacketFeedback(&mut self, packet_feedback: &PacketResult, at_time: Timestamp) {
        // Reset if the stream has timed out.
        if self.last_seen_packet.IsInfinite()
            || at_time - self.last_seen_packet > Self::StreamTimeOut
        {
            self.video_inter_arrival_delta = InterArrivalDelta::new(Self::SendTimeGroupLength);
            self.audio_inter_arrival_delta = InterArrivalDelta::new(Self::SendTimeGroupLength);

            self.video_delay_detector = TrendlineEstimator::default();
            self.audio_delay_detector = TrendlineEstimator::default();
            self.active_delay_detector_type = DelayDetector::Video;
        }
        self.last_seen_packet = at_time;

        // As an alternative to ignoring small packets, we can separate audio and
        // video packets for overuse detection.
        let mut delay_detector_for_packet: DelayDetector = DelayDetector::Video;
        if self.separate_audio.enabled {
            if packet_feedback.sent_packet.audio {
                delay_detector_for_packet = DelayDetector::Audio;
                self.audio_packets_since_last_video += 1;
                if self.audio_packets_since_last_video > self.separate_audio.packet_threshold
                    && packet_feedback.receive_time - self.last_video_packet_recv_time
                        > self.separate_audio.time_threshold
                {
                    self.active_delay_detector_type = DelayDetector::Audio;
                }
            } else {
                self.audio_packets_since_last_video = 0;
                self.last_video_packet_recv_time = std::cmp::max(
                    self.last_video_packet_recv_time,
                    packet_feedback.receive_time,
                );
                self.active_delay_detector_type = DelayDetector::Video;
            }
        }
        let packet_size: DataSize = packet_feedback.sent_packet.size;

        let mut send_delta: TimeDelta = TimeDelta::Zero();
        let mut recv_delta: TimeDelta = TimeDelta::Zero();
        let mut size_delta: i64 = 0;

        let inter_arrival_for_packet: &mut InterArrivalDelta =
            if self.separate_audio.enabled && packet_feedback.sent_packet.audio {
                &mut self.audio_inter_arrival_delta
            } else {
                &mut self.video_inter_arrival_delta
            };
        let calculated_deltas: bool = inter_arrival_for_packet.ComputeDeltas(
            packet_feedback.sent_packet.send_time,
            packet_feedback.receive_time,
            at_time,
            packet_size.bytes() as usize,
            &mut send_delta,
            &mut recv_delta,
            &mut size_delta,
        );

        match delay_detector_for_packet {
            DelayDetector::Audio => self.audio_delay_detector.update(
                recv_delta.ms_float(),
                send_delta.ms_float(),
                packet_feedback.sent_packet.send_time.ms(),
                packet_feedback.receive_time.ms(),
                packet_size.bytes() as usize,
                calculated_deltas,
            ),
            DelayDetector::Video => self.video_delay_detector.update(
                recv_delta.ms_float(),
                send_delta.ms_float(),
                packet_feedback.sent_packet.send_time.ms(),
                packet_feedback.receive_time.ms(),
                packet_size.bytes() as usize,
                calculated_deltas,
            ),
        }
    }

    fn MaybeUpdateEstimate(
        &mut self,
        acked_bitrate: Option<DataRate>,
        probe_bitrate: Option<DataRate>,
        //_state_estimate: Option<NetworkStateEstimate>,
        recovered_from_overuse: bool,
        _in_alr: bool,
        at_time: Timestamp,
    ) -> DelayBasedBweResult {
        let mut result = DelayBasedBweResult::default();

        // Currently overusing the bandwidth.
        if self.active_delay_detector_state() == BandwidthUsage::Overusing {
            if let Some(acked_bitrate) = acked_bitrate {
                if self
                    .rate_control
                    .TimeToReduceFurther(at_time, acked_bitrate)
                {
                    result.updated = self.UpdateEstimate(
                        at_time,
                        Some(acked_bitrate),
                        &mut result.target_bitrate,
                    );
                }
            } else if self.rate_control.ValidEstimate()
                && self.rate_control.InitialTimeToReduceFurther(at_time)
            {
                // Overusing before we have a measured acknowledged bitrate. Reduce send
                // rate by 50% every 200 ms.
                // TODO(tschumim): Improve this and/or the acknowledged bitrate estimator
                // so that we (almost) always have a bitrate estimate.
                self.rate_control
                    .SetEstimate(self.rate_control.LatestEstimate() / 2, at_time);
                result.updated = true;
                result.probe = false;
                result.target_bitrate = self.rate_control.LatestEstimate();
            }
        } else if let Some(probe_bitrate) = probe_bitrate {
            result.probe = true;
            result.updated = true;
            self.rate_control.SetEstimate(probe_bitrate, at_time);
            result.target_bitrate = self.rate_control.LatestEstimate();
        } else {
            result.updated =
                self.UpdateEstimate(at_time, acked_bitrate, &mut result.target_bitrate);
            result.recovered_from_overuse = recovered_from_overuse;
        }
        let detector_state: BandwidthUsage = self.active_delay_detector_state();
        if (result.updated && self.prev_bitrate != result.target_bitrate)
            || detector_state != self.prev_state
        {
            let bitrate: DataRate = if result.updated {
                result.target_bitrate
            } else {
                self.prev_bitrate
            };

            self.prev_bitrate = bitrate;
            self.prev_state = detector_state;
        }

        result.delay_detector_state = detector_state;
        result
    }

    // Updates the current remote rate estimate and returns true if a valid
    // estimate exists.
    fn UpdateEstimate(
        &mut self,
        at_time: Timestamp,
        acked_bitrate: Option<DataRate>,
        target_rate: &mut DataRate,
    ) -> bool {
        let input = RateControlInput::new(self.active_delay_detector_state(), acked_bitrate);
        *target_rate = self.rate_control.Update(input, at_time);
        self.rate_control.ValidEstimate()
    }
}

#[cfg(test)]
mod test {
    use crate::api::transport::PacedPacketInfo;
    use crate::{
        AcknowledgedBitrateEstimator, AcknowledgedBitrateEstimatorInterface, ProbeBitrateEstimator,
        RobustThroughputEstimatorSettings,
    };
    use approx::assert_relative_eq;

    use test_trace::test;

    pub use super::*;

    const DefaultSsrc: u32 = 0;
    const Mtu: usize = 1200;
    const AcceptedBitrateErrorBps: u32 = 50000;

    // Number of packets needed before we have a valid estimate.
    const NumInitialPackets: i64 = 2;

    const InitialProbingPackets: i64 = 5;

    pub struct TestBitrateObserver {
        updated: bool,
        latest_bitrate: u32,
    }

    impl TestBitrateObserver {
        pub fn new() -> Self {
            Self {
                updated: false,
                latest_bitrate: 0,
            }
        }

        pub fn OnReceiveBitrateChanged(&mut self, bitrate: u32) {
            self.latest_bitrate = bitrate;
            self.updated = true;
        }

        pub fn Reset(&mut self) {
            self.updated = false;
        }

        pub fn updated(&self) -> bool {
            self.updated
        }

        pub fn latest_bitrate(&self) -> u32 {
            self.latest_bitrate
        }
    }

    pub struct RtpStream {
        fps: i64,
        bitrate_bps: i64,
        next_rtp_time: i64,
    }

    impl RtpStream {
        const SendSideOffsetUs: i64 = 1000000;

        pub fn new(fps: i64, bitrate_bps: i64) -> Self {
            assert!(fps > 0);
            Self {
                fps,
                bitrate_bps,
                next_rtp_time: 0,
            }
        }

        // Generates a new frame for this stream. If called too soon after the
        // previous frame, no frame will be generated. The frame is split into
        // packets.
        pub fn GenerateFrame(
            &mut self,
            time_now_us: i64,
            next_sequence_number: &mut i64,
            packets: &mut Vec<PacketResult>,
        ) -> i64 {
            if time_now_us < self.next_rtp_time {
                return self.next_rtp_time;
            }
            let bits_per_frame: usize = ((self.bitrate_bps + self.fps / 2) / self.fps) as usize;
            let n_packets: usize = std::cmp::max((bits_per_frame + 4 * Mtu) / (8 * Mtu), 1);
            let payload_size: usize = (bits_per_frame + 4 * n_packets) / (8 * n_packets);
            for i in 0..n_packets {
                let mut packet = PacketResult::default();
                packet.sent_packet.send_time =
                    Timestamp::Micros(time_now_us + Self::SendSideOffsetUs);
                packet.sent_packet.size = DataSize::Bytes(payload_size as _);
                packet.sent_packet.sequence_number = *next_sequence_number;
                *next_sequence_number += 1;
                packets.push(packet);
            }
            self.next_rtp_time = time_now_us + (1000000 + self.fps / 2) / self.fps;
            self.next_rtp_time
        }

        // The send-side time when the next frame can be generated.
        pub fn next_rtp_time(&self) -> i64 {
            self.next_rtp_time
        }

        pub fn set_bitrate_bps(&mut self, bitrate_bps: i64) {
            assert!(bitrate_bps >= 0);
            self.bitrate_bps = bitrate_bps;
        }

        pub fn bitrate_bps(&self) -> i64 {
            self.bitrate_bps
        }
    }

    pub struct StreamGenerator {
        // Capacity of the simulated channel in bits per second.
        capacity: i64,
        // The time when the last packet arrived.
        prev_arrival_time_us: i64,
        // All streams being transmitted on this simulated channel.
        streams: Vec<RtpStream>,
    }

    impl StreamGenerator {
        pub fn new(capacity: i64, time_now: i64) -> Self {
            Self {
                capacity,
                prev_arrival_time_us: time_now,
                streams: Vec::new(),
            }
        }

        // Add a new stream.
        pub fn AddStream(&mut self, stream: RtpStream) {
            self.streams.push(stream);
        }

        // Set the link capacity.
        pub fn set_capacity_bps(&mut self, capacity_bps: i64) {
            assert!(capacity_bps > 0);
            self.capacity = capacity_bps;
        }

        // Divides `bitrate_bps` among all streams. The allocated bitrate per stream
        // is decided by the initial allocation ratios.
        pub fn SetBitrateBps(&mut self, bitrate_bps: i64) {
            assert!(self.streams.len() >= 0);
            let mut total_bitrate_before: i64 = 0;
            for stream in &self.streams {
                total_bitrate_before += stream.bitrate_bps();
            }
            let mut bitrate_before: i64 = 0;
            let mut total_bitrate_after: i64 = 0;
            for stream in &mut self.streams {
                bitrate_before += stream.bitrate_bps();
                let bitrate_after: i64 = (bitrate_before * bitrate_bps + total_bitrate_before / 2)
                    / total_bitrate_before;
                stream.set_bitrate_bps(bitrate_after - total_bitrate_after);
                total_bitrate_after += stream.bitrate_bps();
            }
            assert_eq!(bitrate_before, total_bitrate_before);
            assert_eq!(total_bitrate_after, bitrate_bps);
        }

        // TODO(holmer): Break out the channel simulation part from this class to make
        // it possible to simulate different types of channels.
        pub fn GenerateFrame(
            &mut self,
            time_now_us: i64,
            next_sequence_number: &mut i64,
            packets: &mut Vec<PacketResult>,
        ) -> i64 {
            assert!(packets.is_empty());
            assert!(self.capacity > 0);
            let it = self
                .streams
                .iter_mut()
                .min_by_key(|x| x.next_rtp_time())
                .unwrap();
            it.GenerateFrame(time_now_us, next_sequence_number, packets);
            for packet in packets {
                let capacity_bpus: i64 = self.capacity / 1000;
                let required_network_time_us: i64 = (8 * 1000 * packet.sent_packet.size.bytes()
                    + capacity_bpus / 2)
                    / capacity_bpus;
                self.prev_arrival_time_us = std::cmp::max(
                    time_now_us + required_network_time_us,
                    self.prev_arrival_time_us + required_network_time_us,
                );
                packet.receive_time = Timestamp::Micros(self.prev_arrival_time_us);
            }
            let it = self
                .streams
                .iter()
                .min_by_key(|x| x.next_rtp_time())
                .unwrap();
            it.next_rtp_time().max(time_now_us)
        }
    }

    pub struct DelayBasedBweTest {
        clock: Timestamp, // Time at the receiver.
        bitrate_observer: TestBitrateObserver,
        acknowledged_bitrate_estimator: Box<dyn AcknowledgedBitrateEstimatorInterface>,
        probe_bitrate_estimator: ProbeBitrateEstimator,
        bitrate_estimator: DelayBasedBwe,
        stream_generator: StreamGenerator,
        arrival_time_offset_ms: i64,
        next_sequence_number: i64,
        first_update: bool,
    }

    impl DelayBasedBweTest {
        pub fn new() -> Self {
            let field_trials = FieldTrials {
                robust_throughput_estimator_settings: RobustThroughputEstimatorSettings {
                    enabled: true,
                    ..Default::default()
                },
                ..Default::default()
            };
            let clock = Timestamp::Micros(100000000);

            Self {
                acknowledged_bitrate_estimator: AcknowledgedBitrateEstimator::create(&field_trials),
                probe_bitrate_estimator: ProbeBitrateEstimator::default(),
                bitrate_estimator: DelayBasedBwe::new(&field_trials),
                stream_generator: StreamGenerator::new(
                    1000000, // Capacity.
                    clock.us(),
                ),
                arrival_time_offset_ms: 0,
                next_sequence_number: 0,
                first_update: true,
                clock,
                bitrate_observer: TestBitrateObserver::new(),
            }
        }

        pub fn AddDefaultStream(&mut self) {
            self.stream_generator.AddStream(RtpStream::new(30, 300000));
        }

        // Helpers to insert a single packet into the delay-based BWE.
        /*
        fn IncomingFeedback(arrival_time_ms: i64,
                              send_time_ms: i64,
                              payload_size: usize) {
        IncomingFeedback(arrival_time_ms, send_time_ms, payload_size,
                         PacedPacketInfo::default());
                              }

        fn IncomingFeedback(arrival_time_ms: i64,
                              send_time_ms: i64,
                              payload_size: usize,
                              pacing_info: &PacedPacketInfo) {
        RTC_CHECK_GE(arrival_time_ms + self.arrival_time_offset_ms, 0);
        IncomingFeedback(Timestamp::Millis(arrival_time_ms + self.arrival_time_offset_ms),
                         Timestamp::Millis(send_time_ms), payload_size, pacing_info);
                              }
                              */

        fn IncomingFeedback(
            &mut self,
            arrival_time: Timestamp,
            send_time: Timestamp,
            payload_size: usize,
            pacing_info: PacedPacketInfo,
        ) {
            let receive_time = arrival_time + TimeDelta::Millis(self.arrival_time_offset_ms);

            let mut packet = PacketResult::default();
            packet.receive_time = receive_time;
            packet.sent_packet.send_time = send_time;
            packet.sent_packet.size = DataSize::Bytes(payload_size as _);
            packet.sent_packet.pacing_info = pacing_info;
            packet.sent_packet.sequence_number = self.next_sequence_number;
            self.next_sequence_number += 1;
            if packet.sent_packet.pacing_info.probe_cluster_id != PacedPacketInfo::NotAProbe {
                self.probe_bitrate_estimator
                    .HandleProbeAndEstimateBitrate(&packet);
            }

            let mut msg = TransportPacketsFeedback::default();
            msg.feedback_time = Timestamp::Millis(self.clock.ms());
            msg.packet_feedbacks.push(packet);
            self.acknowledged_bitrate_estimator
                .incoming_packet_feedback(&msg.SortedByReceiveTime());
            let result = self.bitrate_estimator.IncomingPacketFeedbackVector(
                &msg,
                self.acknowledged_bitrate_estimator.bitrate(),
                self.probe_bitrate_estimator
                    .FetchAndResetLastEstimatedBitrate(),
                /*in_alr*/ false,
            );
            if result.updated {
                self.bitrate_observer
                    .OnReceiveBitrateChanged(result.target_bitrate.bps() as _);
            }
        }

        // Generates a frame of packets belonging to a stream at a given bitrate and
        // with a given ssrc. The stream is pushed through a very simple simulated
        // network, and is then given to the receive-side bandwidth estimator.
        // Returns true if an over-use was seen, false otherwise.
        // The StreamGenerator::updated() should be used to check for any changes in
        // target bitrate after the call to this function.
        fn GenerateAndProcessFrame(&mut self, ssrc: u32, bitrate_bps: u32) -> bool {
            self.stream_generator.SetBitrateBps(bitrate_bps as _);
            let mut packets: Vec<PacketResult> = Vec::new();

            let next_time_us: i64 = self.stream_generator.GenerateFrame(
                self.clock.us(),
                &mut self.next_sequence_number,
                &mut packets,
            );
            if packets.is_empty() {
                return false;
            }

            let mut overuse: bool = false;
            self.bitrate_observer.Reset();
            self.clock = packets.last().unwrap().receive_time;
            for packet in &mut packets {
                assert!(packet.receive_time.ms() + self.arrival_time_offset_ms >= 0);
                packet.receive_time += TimeDelta::Millis(self.arrival_time_offset_ms);

                if packet.sent_packet.pacing_info.probe_cluster_id != PacedPacketInfo::NotAProbe {
                    self.probe_bitrate_estimator
                        .HandleProbeAndEstimateBitrate(packet);
                }
            }

            self.acknowledged_bitrate_estimator
                .incoming_packet_feedback(&packets);
            let mut msg = TransportPacketsFeedback::default();
            msg.packet_feedbacks = packets;
            msg.feedback_time = self.clock;

            let result: DelayBasedBweResult = self.bitrate_estimator.IncomingPacketFeedbackVector(
                &msg,
                self.acknowledged_bitrate_estimator.bitrate(),
                self.probe_bitrate_estimator
                    .FetchAndResetLastEstimatedBitrate(),
                /*in_alr*/ false,
            );
            if result.updated {
                self.bitrate_observer
                    .OnReceiveBitrateChanged(result.target_bitrate.bps() as _);
                if !self.first_update && result.target_bitrate.bps() < bitrate_bps as _ {
                    overuse = true;
                }
                self.first_update = false;
            }

            self.clock = Timestamp::Micros(next_time_us);
            overuse
        }

        // Run the bandwidth estimator with a stream of `number_of_frames` frames, or
        // until it reaches `target_bitrate`.
        // Can for instance be used to run the estimator for some time to get it
        // into a steady state.
        fn SteadyStateRun(
            &mut self,
            ssrc: u32,
            number_of_frames: i64,
            start_bitrate: u32,
            min_bitrate: u32,
            max_bitrate: u32,
            target_bitrate: u32,
        ) -> u32 {
            let mut bitrate_bps: u32 = start_bitrate;
            let mut bitrate_update_seen: bool = false;
            // Produce `number_of_frames` frames and give them to the estimator.
            for i in 0..number_of_frames {
                let overuse: bool = self.GenerateAndProcessFrame(ssrc, bitrate_bps);
                if overuse {
                    assert!(self.bitrate_observer.latest_bitrate() < max_bitrate);
                    assert!(self.bitrate_observer.latest_bitrate() > min_bitrate);
                    bitrate_bps = self.bitrate_observer.latest_bitrate();
                    bitrate_update_seen = true;
                } else if self.bitrate_observer.updated() {
                    bitrate_bps = self.bitrate_observer.latest_bitrate();
                    self.bitrate_observer.Reset();
                }
                if bitrate_update_seen && bitrate_bps > target_bitrate {
                    break;
                }
            }
            assert!(bitrate_update_seen);
            bitrate_bps
        }

        fn TestTimestampGroupingTestHelper(&mut self) {
            const Framerate: i64 = 50; // 50 fps to avoid rounding errors.
            const FrameIntervalMs: i64 = 1000 / Framerate;
            let mut send_time_ms: i64 = 0;
            // Initial set of frames to increase the bitrate. 6 seconds to have enough
            // time for the first estimate to be generated and for Process() to be called.
            for i in 0..=(6*Framerate) {
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms),
                    1000,
                    Default::default(),
                );

                self.clock += TimeDelta::Millis(FrameIntervalMs as _);
                send_time_ms += FrameIntervalMs;
            }
            assert!(self.bitrate_observer.updated());
            assert!(self.bitrate_observer.latest_bitrate() >= 400000);

            // Insert batches of frames which were sent very close in time. Also simulate
            // capacity over-use to see that we back off correctly.
            const TimestampGroupLength: i64 = 15;
            for i in 0..100 {
                for j in 0..TimestampGroupLength {
                    // Insert `kTimestampGroupLength` frames with just 1 timestamp ticks in
                    // between. Should be treated as part of the same group by the estimator.
                    self.IncomingFeedback(
                        self.clock,
                        Timestamp::Millis(send_time_ms),
                        100,
                        Default::default(),
                    );
                    self.clock += TimeDelta::Millis(FrameIntervalMs / TimestampGroupLength);
                    send_time_ms += 1;
                }
                // Increase time until next batch to simulate over-use.
                self.clock += TimeDelta::Millis(10);
                send_time_ms += FrameIntervalMs - TimestampGroupLength;
            }
            assert!(self.bitrate_observer.updated());
            // Should have reduced the estimate.
            assert!(self.bitrate_observer.latest_bitrate() < 400000);
        }

        fn TestWrappingHelper(&mut self, silence_time_s: i64) {
            const Framerate: i64 = 100;
            const FrameIntervalMs: i64 = 1000 / Framerate;
            let mut send_time_ms: i64 = 0;

            for i in 0..3000 {
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms),
                    1000,
                    Default::default(),
                );
                self.clock += TimeDelta::Millis(FrameIntervalMs);
                send_time_ms += FrameIntervalMs;
            }
            let mut bitrate_before: DataRate = DataRate::Zero();
            let mut ssrcs = Vec::new();
            self.bitrate_estimator
                .LatestEstimate(&mut ssrcs, &mut bitrate_before);

            self.clock += TimeDelta::Millis(silence_time_s * 1000);
            send_time_ms += silence_time_s * 1000;

            for i in 0..24 {
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms),
                    1000,
                    Default::default(),
                );
                self.clock += TimeDelta::Millis(2 * FrameIntervalMs);
                send_time_ms += FrameIntervalMs;
            }
            let mut bitrate_after: DataRate = DataRate::Zero();
            self.bitrate_estimator
                .LatestEstimate(&mut ssrcs, &mut bitrate_after);
            assert!(bitrate_after < bitrate_before);
        }

        fn InitialBehaviorTestHelper(&mut self, expected_converge_bitrate: u32) {
            const Framerate: i64 = 50; // 50 fps to avoid rounding errors.
            const FrameIntervalMs: i64 = 1000 / Framerate;
            const PacingInfo: PacedPacketInfo = PacedPacketInfo::new(0, 5, 5000);
            let mut bitrate: DataRate = DataRate::Zero();
            let mut send_time_ms: i64 = 0;
            let mut ssrcs: Vec<u32> = Vec::new();
            assert!(!self
                .bitrate_estimator
                .LatestEstimate(&mut ssrcs, &mut bitrate));
            assert_eq!(0, ssrcs.len());
            self.clock += TimeDelta::Millis(1000);
            assert!(!self
                .bitrate_estimator
                .LatestEstimate(&mut ssrcs, &mut bitrate));
            assert!(!self.bitrate_observer.updated());
            self.bitrate_observer.Reset();
            self.clock += TimeDelta::Millis(1000);
            // Inserting packets for 5 seconds to get a valid estimate.
            for i in 0..(5 * Framerate + 1 + NumInitialPackets) {
                // NOTE!!! If the following line is moved under the if case then this test
                //         wont work on windows realease bots.
                let pacing_info: PacedPacketInfo = if i < InitialProbingPackets {
                    PacingInfo
                } else {
                    PacedPacketInfo::default()
                };

                if i == NumInitialPackets {
                    assert!(!self
                        .bitrate_estimator
                        .LatestEstimate(&mut ssrcs, &mut bitrate));
                    assert_eq!(0, ssrcs.len());
                    assert!(!self.bitrate_observer.updated());
                    self.bitrate_observer.Reset();
                }
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms),
                    Mtu,
                    pacing_info,
                );
                self.clock += TimeDelta::Millis(1000 / Framerate);
                send_time_ms += FrameIntervalMs;
            }
            assert!(self
                .bitrate_estimator
                .LatestEstimate(&mut ssrcs, &mut bitrate));
            assert_eq!(1, ssrcs.len());
            assert_eq!(&DefaultSsrc, ssrcs.first().unwrap());
            assert_relative_eq!(
                expected_converge_bitrate as f64,
                bitrate.bps_float(),
                epsilon = AcceptedBitrateErrorBps as f64
            );
            assert!(self.bitrate_observer.updated());
            self.bitrate_observer.Reset();
            assert_eq!(self.bitrate_observer.latest_bitrate() as i64, bitrate.bps());
        }

        fn RateIncreaseReorderingTestHelper(&mut self, expected_bitrate_bps: u32) {
            const Framerate: i64 = 50; // 50 fps to avoid rounding errors.
            const FrameIntervalMs: i64 = 1000 / Framerate;
            const PacingInfo: PacedPacketInfo = PacedPacketInfo::new(0, 5, 5000);
            let mut send_time_ms: i64 = 0;
            // Inserting packets for five seconds to get a valid estimate.
            for i in 0..(5 * Framerate + 1 + NumInitialPackets) {
                // NOTE!!! If the following line is moved under the if case then this test
                //         wont work on windows realease bots.
                let pacing_info: PacedPacketInfo = if i < InitialProbingPackets {
                    PacingInfo
                } else {
                    PacedPacketInfo::default()
                };

                // TODO(sprang): Remove this hack once the single stream estimator is gone,
                // as it doesn't do anything in Process().
                if i == NumInitialPackets {
                    // Process after we have enough frames to get a valid input rate estimate.

                    assert!(!self.bitrate_observer.updated()); // No valid estimate.
                }
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms),
                    Mtu,
                    pacing_info,
                );
                self.clock += TimeDelta::Millis(FrameIntervalMs);
                send_time_ms += FrameIntervalMs;
            }
            assert!(self.bitrate_observer.updated());
            assert_relative_eq!(
                expected_bitrate_bps as f64,
                self.bitrate_observer.latest_bitrate() as f64,
                epsilon = AcceptedBitrateErrorBps as f64
            );
            for i in 0..10 {
                self.clock += TimeDelta::Millis(2 * FrameIntervalMs);
                send_time_ms += 2 * FrameIntervalMs;
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms),
                    1000,
                    Default::default(),
                );
                self.IncomingFeedback(
                    self.clock,
                    Timestamp::Millis(send_time_ms - FrameIntervalMs),
                    1000,
                    Default::default(),
                );
            }
            assert!(self.bitrate_observer.updated());
            assert_relative_eq!(
                expected_bitrate_bps as f64,
                self.bitrate_observer.latest_bitrate() as f64,
                epsilon = AcceptedBitrateErrorBps as f64
            );
        }
        fn RateIncreaseRtpTimestampsTestHelper(&mut self, expected_iterations: i64) {
            // This threshold corresponds approximately to increasing linearly with
            // bitrate(i) = 1.04 * bitrate(i-1) + 1000
            // until bitrate(i) > 500000, with bitrate(1) ~= 30000.
            let mut bitrate_bps: u32 = 30000;
            let mut iterations: i64 = 0;
            self.AddDefaultStream();
            // Feed the estimator with a stream of packets and verify that it reaches
            // 500 kbps at the expected time.
            while bitrate_bps < 500000 {
                let overuse: bool = self.GenerateAndProcessFrame(DefaultSsrc, bitrate_bps);
                if overuse {
                    assert!(self.bitrate_observer.latest_bitrate() > bitrate_bps);
                    bitrate_bps = self.bitrate_observer.latest_bitrate();
                    self.bitrate_observer.Reset();
                } else if self.bitrate_observer.updated() {
                    bitrate_bps = self.bitrate_observer.latest_bitrate();
                    self.bitrate_observer.Reset();
                }
                iterations += 1;
            }
            assert_eq!(expected_iterations, iterations);
        }
        fn CapacityDropTestHelper(
            &mut self,
            number_of_streams: i64,
            wrap_time_stamp: bool,
            expected_bitrate_drop_delta: u32,
            receiver_clock_offset_change_ms: i64,
        ) {
            const Framerate: i64 = 30;
            const StartBitrate: i64 = 900000;
            const MinExpectedBitrate: i64 = 800000;
            const MaxExpectedBitrate: i64 = 1100000;
            const InitialCapacityBps: u32 = 1000000;
            const ReducedCapacityBps: u32 = 500000;

            let mut steady_state_time: i64 = 0;
            if number_of_streams <= 1 {
                steady_state_time = 10;
                self.AddDefaultStream();
            } else {
                steady_state_time = 10 * number_of_streams;
                let mut bitrate_sum: i64 = 0;
                let BitrateDenom: i64 = number_of_streams * (number_of_streams - 1);
                for i in 0..number_of_streams {
                    // First stream gets half available bitrate, while the rest share the
                    // remaining half i.e.: 1/2 = Sum[n/(N*(N-1))] for n=1..N-1 (rounded up)
                    let mut bitrate: i64 = StartBitrate / 2;
                    if i > 0 {
                        bitrate = (StartBitrate * i + BitrateDenom / 2) / BitrateDenom;
                    }
                    self.stream_generator
                        .AddStream(RtpStream::new(Framerate, bitrate));
                    bitrate_sum += bitrate;
                }
                assert_eq!(bitrate_sum, StartBitrate);
            }

            // Run in steady state to make the estimator converge.
            self.stream_generator
                .set_capacity_bps(InitialCapacityBps as _);
            let mut bitrate_bps: u32 = self.SteadyStateRun(
                DefaultSsrc,
                steady_state_time * Framerate,
                StartBitrate as _,
                MinExpectedBitrate as _,
                MaxExpectedBitrate as _,
                InitialCapacityBps,
            );
            assert_relative_eq!(
                InitialCapacityBps as f64,
                bitrate_bps as f64,
                epsilon = 180000.0
            );
            self.bitrate_observer.Reset();

            // Add an offset to make sure the BWE can handle it.
            self.arrival_time_offset_ms += receiver_clock_offset_change_ms;

            // Reduce the capacity and verify the decrease time.
            self.stream_generator
                .set_capacity_bps(ReducedCapacityBps as _);
            let overuse_start_time: i64 = self.clock.ms();
            let mut bitrate_drop_time: i64 = -1;
            for i in 0..(100 * number_of_streams) {
                self.GenerateAndProcessFrame(DefaultSsrc, bitrate_bps);
                if bitrate_drop_time == -1
                    && self.bitrate_observer.latest_bitrate() <= ReducedCapacityBps
                {
                    bitrate_drop_time = self.clock.ms();
                }
                if self.bitrate_observer.updated() {
                    bitrate_bps = self.bitrate_observer.latest_bitrate();
                }
            }

            assert_relative_eq!(
                expected_bitrate_drop_delta as f64,
                (bitrate_drop_time - overuse_start_time) as f64,
                epsilon = 33.0
            );
        }
    }

    const NumProbesCluster0: i64 = 5;
    const NumProbesCluster1: i64 = 8;
    const PacingInfo0: PacedPacketInfo = PacedPacketInfo::new(0, NumProbesCluster0, 2000);
    const PacingInfo1: PacedPacketInfo = PacedPacketInfo::new(1, NumProbesCluster1, 4000);
    const TargetUtilizationFraction: f64 = 0.95;

    #[test]
    fn ProbeDetection() {
        let mut bwe = DelayBasedBweTest::new();

        // First burst sent at 8 * 1000 / 10 = 800 kbps.
        for i in 0..NumProbesCluster0 {
            bwe.clock += TimeDelta::Millis(10);
            bwe.IncomingFeedback(bwe.clock, bwe.clock, 1000, PacingInfo0);
        }
        assert!(bwe.bitrate_observer.updated());

        // Second burst sent at 8 * 1000 / 5 = 1600 kbps.
        for i in 0..NumProbesCluster1 {
            bwe.clock += TimeDelta::Millis(5);
            bwe.IncomingFeedback(bwe.clock, bwe.clock, 1000, PacingInfo1);
        }

        assert!(bwe.bitrate_observer.updated());
        assert!(bwe.bitrate_observer.latest_bitrate() > 1500000);
    }

    #[test]
    fn ProbeDetectionNonPacedPackets() {
        let mut bwe = DelayBasedBweTest::new();

        // First burst sent at 8 * 1000 / 10 = 800 kbps, but with every other packet
        // not being paced which could mess things up.
        for i in 0..NumProbesCluster0 {
            bwe.clock += TimeDelta::Millis(5);

            bwe.IncomingFeedback(bwe.clock, bwe.clock, 1000, PacingInfo0);
            // Non-paced packet, arriving 5 ms after.
            bwe.clock += TimeDelta::Millis(5);
            bwe.IncomingFeedback(bwe.clock, bwe.clock, 100, PacedPacketInfo::default());
        }

        assert!(bwe.bitrate_observer.updated());
        assert!(bwe.bitrate_observer.latest_bitrate() > 800000);
    }

    #[test]
    fn ProbeDetectionFasterArrival() {
        let mut bwe = DelayBasedBweTest::new();

        // First burst sent at 8 * 1000 / 10 = 800 kbps.
        // Arriving at 8 * 1000 / 5 = 1600 kbps.
        let mut send_time_ms: Timestamp = Timestamp::Zero();
        for i in 0..NumProbesCluster0 {
            bwe.clock += TimeDelta::Millis(1);
            send_time_ms += TimeDelta::Millis(10);

            bwe.IncomingFeedback(bwe.clock, send_time_ms, 1000, PacingInfo0);
        }

        assert!(!bwe.bitrate_observer.updated());
    }

    #[test]
    fn ProbeDetectionSlowerArrival() {
        let mut bwe = DelayBasedBweTest::new();

        // First burst sent at 8 * 1000 / 5 = 1600 kbps.
        // Arriving at 8 * 1000 / 7 = 1142 kbps.
        // Since the receive rate is significantly below the send rate, we expect to
        // use 95% of the estimated capacity.
        let mut send_time_ms: Timestamp = Timestamp::Zero();
        for i in 0..NumProbesCluster1 {
            bwe.clock += TimeDelta::Millis(7);
            send_time_ms += TimeDelta::Millis(5);

            bwe.IncomingFeedback(bwe.clock, send_time_ms, 1000, PacingInfo1);
        }

        assert!(bwe.bitrate_observer.updated());
        assert_relative_eq!(
            bwe.bitrate_observer.latest_bitrate() as f64,
            { (TargetUtilizationFraction * 1140000.0) },
            epsilon = 10000.0
        );
    }

    #[test]
    fn ProbeDetectionSlowerArrivalHighBitrate() {
        let mut bwe = DelayBasedBweTest::new();

        // Burst sent at 8 * 1000 / 1 = 8000 kbps.
        // Arriving at 8 * 1000 / 2 = 4000 kbps.
        // Since the receive rate is significantly below the send rate, we expect to
        // use 95% of the estimated capacity.
        let mut send_time_ms: Timestamp = Timestamp::Zero();
        for i in 0..NumProbesCluster1 {
            bwe.clock += TimeDelta::Millis(2);
            send_time_ms += TimeDelta::Millis(1);

            bwe.IncomingFeedback(bwe.clock, send_time_ms, 1000, PacingInfo1);
        }

        assert!(bwe.bitrate_observer.updated());
        assert_relative_eq!(
            bwe.bitrate_observer.latest_bitrate() as f64,
            TargetUtilizationFraction * 4000000.0,
            epsilon = 10000.0
        );
    }

    #[test]
    fn GetExpectedBwePeriodMs() {
        let mut bwe = DelayBasedBweTest::new();
        let default_interval = bwe.bitrate_estimator.GetExpectedBwePeriod();
        assert!(default_interval.ms() > 0);
        bwe.CapacityDropTestHelper(1, true, 533, 0);
        let interval = bwe.bitrate_estimator.GetExpectedBwePeriod();
        assert!(interval.ms() > 0);
        assert_ne!(interval.ms(), default_interval.ms());
    }

    #[test]
    fn InitialBehavior() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.InitialBehaviorTestHelper(730000);
    }

    #[test]
    fn InitializeResult() {
        let result = DelayBasedBweResult::default();
        assert_eq!(result.delay_detector_state, BandwidthUsage::Normal);
    }

    #[test]
    fn RateIncreaseReordering() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.RateIncreaseReorderingTestHelper(730000);
    }
    #[test]
    fn RateIncreaseRtpTimestamps() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.RateIncreaseRtpTimestampsTestHelper(617);
    }

    #[test]
    fn CapacityDropOneStream() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.CapacityDropTestHelper(1, false, 500, 0);
    }

    #[test]
    fn CapacityDropPosOffsetChange() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.CapacityDropTestHelper(1, false, 867, 30000);
    }

    #[test]
    fn CapacityDropNegOffsetChange() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.CapacityDropTestHelper(1, false, 933, -30000);
    }

    #[test]
    fn CapacityDropOneStreamWrap() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.CapacityDropTestHelper(1, true, 533, 0);
    }

    #[test]
    fn TestTimestampGrouping() {
        let mut bwe = DelayBasedBweTest::new();
        bwe.TestTimestampGroupingTestHelper();
    }

    #[test]
    fn TestShortTimeoutAndWrap() {
        let mut bwe = DelayBasedBweTest::new();
        // Simulate a client leaving and rejoining the call after 35 seconds. This
        // will make abs send time wrap, so if streams aren't timed out properly
        // the next 30 seconds of packets will be out of order.
        bwe.TestWrappingHelper(35);
    }

    #[test]
    fn TestLongTimeoutAndWrap() {
        let mut bwe = DelayBasedBweTest::new();
        // Simulate a client leaving and rejoining the call after some multiple of
        // 64 seconds later. This will cause a zero difference in abs send times due
        // to the wrap, but a big difference in arrival time, if streams aren't
        // properly timed out.
        bwe.TestWrappingHelper(10 * 64);
    }

    #[test]
    fn TestInitialOveruse() {
        let mut bwe = DelayBasedBweTest::new();
        const StartBitrate: DataRate = DataRate::KilobitsPerSec(300);
        const InitialCapacity: DataRate = DataRate::KilobitsPerSec(200);
        const DummySsrc: u32 = 0;
        // High FPS to ensure that we send a lot of packets in a short time.
        const Fps: i64 = 90;

        bwe.stream_generator
            .AddStream(RtpStream::new(Fps, StartBitrate.bps()));
        bwe.stream_generator.set_capacity_bps(InitialCapacity.bps());

        // Needed to initialize the AimdRateControl.
        bwe.bitrate_estimator.SetStartBitrate(StartBitrate);

        // Produce 40 frames (in 1/3 second) and give them to the estimator.
        let mut bitrate_bps: i64 = StartBitrate.bps();
        let mut seen_overuse: bool = false;
        for i in 0..40 {
            let overuse: bool = bwe.GenerateAndProcessFrame(DummySsrc, bitrate_bps as _);
            if overuse {
                assert!(bwe.bitrate_observer.updated());
                assert!(bwe.bitrate_observer.latest_bitrate() as i64 <= InitialCapacity.bps());
                assert!(
                    bwe.bitrate_observer.latest_bitrate() as f64
                        > 0.8 * InitialCapacity.bps_float()
                );
                bitrate_bps = bwe.bitrate_observer.latest_bitrate() as _;
                seen_overuse = true;
                break;
            } else if bwe.bitrate_observer.updated() {
                bitrate_bps = bwe.bitrate_observer.latest_bitrate() as _;
                bwe.bitrate_observer.Reset();
            }
        }
        assert!(seen_overuse);
        assert!(bwe.bitrate_observer.latest_bitrate() as i64 <= InitialCapacity.bps());
        assert!(bwe.bitrate_observer.latest_bitrate() as f64 > 0.8 * InitialCapacity.bps_float());
    }

    #[test]
    fn TestTimestampPrecisionHandling() {
        let mut bwe = DelayBasedBweTest::new();
        // This test does some basic checks to make sure that timestamps with higher
        // than millisecond precision are handled properly and do not cause any
        // problems in the estimator. Specifically, previously reported in
        // webrtc:14023 and described in more details there, the rounding to the
        // nearest milliseconds caused discrepancy in the accumulated delay. This lead
        // to false-positive overuse detection.
        // Technical details of the test:
        // Send times(ms): 0.000,  9.725, 20.000, 29.725, 40.000, 49.725, ...
        // Recv times(ms): 0.500, 10.000, 20.500, 30.000, 40.500, 50.000, ...
        // Send deltas(ms):   9.750,  10.250,  9.750, 10.250,  9.750, ...
        // Recv deltas(ms):   9.500,  10.500,  9.500, 10.500,  9.500, ...
        // There is no delay building up between the send times and the receive times,
        // therefore this case should never lead to an overuse detection. However, if
        // the time deltas were accidentally rounded to the nearest milliseconds, then
        // all the send deltas would be equal to 10ms while some recv deltas would
        // round up to 11ms which would lead in a false illusion of delay build up.
        let mut last_bitrate: u32 = bwe.bitrate_observer.latest_bitrate();
        for i in 0..1000 {
            bwe.clock += TimeDelta::Micros(500);
            bwe.IncomingFeedback(
                bwe.clock,
                bwe.clock - TimeDelta::Micros(500),
                1000,
                PacedPacketInfo::default(),
            );
            bwe.clock += TimeDelta::Micros(9500);
            bwe.IncomingFeedback(
                bwe.clock,
                bwe.clock - TimeDelta::Micros(250),
                1000,
                PacedPacketInfo::default(),
            );
            bwe.clock += TimeDelta::Micros(10000);

            // The bitrate should never decrease in this test.
            assert!(last_bitrate <= bwe.bitrate_observer.latest_bitrate());
            last_bitrate = bwe.bitrate_observer.latest_bitrate();
        }
    }
} // namespace webrtc
