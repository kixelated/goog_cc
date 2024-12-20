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
        transport::{BandwidthUsage, NetworkStateEstimate, PacketResult, TransportPacketsFeedback},
        units::{DataRate, DataSize, TimeDelta, Timestamp},
    },
    remote_bitrate_estimator::{AimdRateControl, RateControlInput},
    DelayIncreaseDetectorInterface, InterArrivalDelta, TrendlineEstimator,
    TrendlineEstimatorSettings,
};

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

    pub fn new(settings: BweSeparateAudioPacketsSettings) -> Self {
        tracing::info!("Initialized DelayBasedBwe with separate audio overuse detection");
        Self {
            separate_audio: settings,
            audio_packets_since_last_video: 0,
            last_video_packet_recv_time: Timestamp::MinusInfinity(),
            video_delay_detector: TrendlineEstimator::default(),
            audio_delay_detector: TrendlineEstimator::default(),
            active_delay_detector_type: DelayDetector::Video,
            last_seen_packet: Timestamp::MinusInfinity(),
            uma_recorded: false,
            rate_control: AimdRateControl::new(true), // send_side
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
        network_estimate: Option<NetworkStateEstimate>,
        in_alr: bool,
    ) -> DelayBasedBweResult {
        let packet_feedback_vector = msg.SortedByReceiveTime();
        // TODO(holmer): An empty feedback vector here likely means that
        // all acks were too late and that the send time history had
        // timed out. We should reduce the rate when this occurs.
        if (packet_feedback_vector.is_empty()) {
            tracing::warn!("Very late feedback received.");
            return DelayBasedBweResult::default();
        }

        if (!self.uma_recorded) {
            self.uma_recorded = true;
        }
        let mut delayed_feedback: bool = true;
        let mut recovered_from_overuse: bool = false;
        let mut prev_detector_state: BandwidthUsage = self.active_delay_detector_state();
        for packet_feedback in &packet_feedback_vector {
            delayed_feedback = false;
            self.IncomingPacketFeedback(packet_feedback, msg.feedback_time);
            if (prev_detector_state == BandwidthUsage::Underusing
                && self.active_delay_detector_state() == BandwidthUsage::Normal)
            {
                recovered_from_overuse = true;
            }
            prev_detector_state = self.active_delay_detector_state();
        }

        if (delayed_feedback) {
            // TODO(bugs.webrtc.org/10125): Design a better mechanism to safe-guard
            // against building very large network queues.
            return DelayBasedBweResult::default();
        }
        self.rate_control.SetInApplicationLimitedRegion(in_alr);
        self.rate_control.SetNetworkStateEstimate(network_estimate);
        return self.MaybeUpdateEstimate(
            acked_bitrate,
            probe_bitrate,
            network_estimate,
            recovered_from_overuse,
            in_alr,
            msg.feedback_time,
        );
    }

    pub fn OnRttUpdate(&mut self, avg_rtt: TimeDelta) {
        self.rate_control.SetRtt(avg_rtt);
    }
    pub fn LatestEstimate(&self, ssrcs: &mut Vec<u32>, bitrate: &mut DataRate) -> bool {
        // Currently accessed from both the process thread (see
        // ModuleRtpRtcpImpl::Process()) and the configuration thread (see
        // Call::GetStats()). Should in the future only be accessed from a single
        // thread.
        if (!self.rate_control.ValidEstimate()) {
            return false;
        }

        *ssrcs = vec![Self::FixedSsrc];
        *bitrate = self.rate_control.LatestEstimate();
        return true;
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
        return self.rate_control.GetExpectedBandwidthPeriod();
    }
    pub fn TriggerOveruse(
        &mut self,
        at_time: Timestamp,
        link_capacity: Option<DataRate>,
    ) -> DataRate {
        let input = RateControlInput::new(BandwidthUsage::Overusing, link_capacity);
        return self.rate_control.Update(input, at_time);
    }
    pub fn last_estimate(&self) -> DataRate {
        return self.prev_bitrate;
    }
    pub fn last_state(&self) -> BandwidthUsage {
        return self.prev_state;
    }

    fn IncomingPacketFeedback(&mut self, packet_feedback: &PacketResult, at_time: Timestamp) {
        // Reset if the stream has timed out.
        if (self.last_seen_packet.IsInfinite()
            || at_time - self.last_seen_packet > Self::StreamTimeOut)
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
        if (self.separate_audio.enabled) {
            if (packet_feedback.sent_packet.audio) {
                delay_detector_for_packet = DelayDetector::Audio;
                self.audio_packets_since_last_video += 1;
                if (self.audio_packets_since_last_video > self.separate_audio.packet_threshold
                    && packet_feedback.receive_time - self.last_video_packet_recv_time
                        > self.separate_audio.time_threshold)
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
        let mut size_delta: isize = 0;

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
        state_estimate: Option<NetworkStateEstimate>,
        recovered_from_overuse: bool,
        in_alr: bool,
        at_time: Timestamp,
    ) -> DelayBasedBweResult {
        let mut result = DelayBasedBweResult::default();

        // Currently overusing the bandwidth.
        if (self.active_delay_detector_state() == BandwidthUsage::Overusing) {
            if let Some(acked_bitrate) = acked_bitrate {
                if (self
                    .rate_control
                    .TimeToReduceFurther(at_time, acked_bitrate))
                {
                    result.updated = self.UpdateEstimate(
                        at_time,
                        Some(acked_bitrate),
                        &mut result.target_bitrate,
                    );
                }
            } else if (self.rate_control.ValidEstimate()
                && self.rate_control.InitialTimeToReduceFurther(at_time))
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
        } else {
            if let Some(probe_bitrate) = probe_bitrate {
                result.probe = true;
                result.updated = true;
                self.rate_control.SetEstimate(probe_bitrate, at_time);
                result.target_bitrate = self.rate_control.LatestEstimate();
            } else {
                result.updated =
                    self.UpdateEstimate(at_time, acked_bitrate, &mut result.target_bitrate);
                result.recovered_from_overuse = recovered_from_overuse;
            }
        }
        let detector_state: BandwidthUsage = self.active_delay_detector_state();
        if ((result.updated && self.prev_bitrate != result.target_bitrate)
            || detector_state != self.prev_state)
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
        return result;
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
        return self.rate_control.ValidEstimate();
    }
}
