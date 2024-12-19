/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"

#include <algorithm>
#include <cstdint>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include "api/field_trials_view.h"
#include "api/network_state_predictor.h"
#include "api/rtc_event_log/rtc_event_log.h"
#include "api/transport/bandwidth_usage.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "logging/rtc_event_log/events/rtc_event_bwe_update_delay_based.h"
#include "modules/congestion_controller/goog_cc/delay_increase_detector_interface.h"
#include "modules/congestion_controller/goog_cc/inter_arrival_delta.h"
#include "modules/congestion_controller/goog_cc/trendline_estimator.h"
#include "modules/remote_bitrate_estimator/include/bwe_defines.h"
#include "rtc_base/checks.h"
#include "rtc_base/experiments/struct_parameters_parser.h"
#include "rtc_base/logging.h"
#include "rtc_base/race_checker.h"
#include "system_wrappers/include/metrics.h"


namespace {
const StreamTimeOut: TimeDelta = Duration::from_secs(2);
const SendTimeGroupLength: TimeDelta = TimeDelta::Millis(5);

// This ssrc is used to fulfill the current API but will be removed
// after the API has been changed.
const FixedSsrc: u32 = 0;
}  // namespace

constexpr char BweSeparateAudioPacketsSettings::kKey[];

BweSeparateAudioPacketsSettings::BweSeparateAudioPacketsSettings(
    const FieldTrialsView* key_value_config) {
  Parser()->Parse(
      key_value_config->Lookup(BweSeparateAudioPacketsSettings::kKey));
}

std::unique_ptr<StructParametersParser>
BweSeparateAudioPacketsSettings::Parser() {
  return StructParametersParser::Create(      //
      "enabled", &enabled,                    //
      "packet_threshold", &packet_threshold,  //
      "time_threshold", &time_threshold);
}

DelayBasedBwe::Result::Result()
    : updated(false),
      probe(false),
      target_bitrate(DataRate::Zero()),
      recovered_from_overuse(false),
      delay_detector_state(BandwidthUsage::kBwNormal) {}

DelayBasedBwe::DelayBasedBwe(const FieldTrialsView* key_value_config,
                             RtcEventLog* event_log,
                             NetworkStatePredictor* network_state_predictor)
    : event_log_(event_log),
      key_value_config_(key_value_config),
      separate_audio_(key_value_config),
      audio_packets_since_last_video_(0),
      last_video_packet_recv_time_(Timestamp::MinusInfinity()),
      network_state_predictor_(network_state_predictor),
      video_delay_detector_(
          new TrendlineEstimator(self.key_value_config, self.network_state_predictor)),
      audio_delay_detector_(
          new TrendlineEstimator(self.key_value_config, self.network_state_predictor)),
      active_delay_detector_(self.video_delay_detector.get()),
      last_seen_packet_(Timestamp::MinusInfinity()),
      uma_recorded_(false),
      rate_control_(*key_value_config, /*send_side=*/true),
      prev_bitrate_(DataRate::Zero()),
      prev_state_(BandwidthUsage::kBwNormal) {
  RTC_LOG(LS_INFO)
      << "Initialized DelayBasedBwe with separate audio overuse detection"
      << self.separate_audio.Parser()->Encode();
}

DelayBasedBwe::~DelayBasedBwe() {}

DelayBasedBwe::Result IncomingPacketFeedbackVector(&self /* DelayBasedBwe */,
    const TransportPacketsFeedback& msg,
    Option<DataRate> acked_bitrate,
    Option<DataRate> probe_bitrate,
    Option<NetworkStateEstimate> network_estimate,
    bool in_alr) {
  assert!_RUNS_SERIALIZED(&self.network_race);

let packet_feedback_vector = msg.SortedByReceiveTime();
  // TODO(holmer): An empty feedback vector here likely means that
  // all acks were too late and that the send time history had
  // timed out. We should reduce the rate when this occurs.
  if (packet_feedback_vector.empty()) {
    tracing::warn!( "Very late feedback received.");
    return DelayBasedBwe::Result();
  }

  if (!self.uma_recorded) {
    RTC_HISTOGRAM_ENUMERATION(kBweTypeHistogram,
                              BweNames::kSendSideTransportSeqNum,
                              BweNames::kBweNamesMax);
    self.uma_recorded = true;
  }
  let delayed_feedback: bool = true;
  let recovered_from_overuse: bool = false;
  let prev_detector_state: BandwidthUsage = self.active_delay_detector.State();
  for packet_feedback in &packet_feedback_vector {
    delayed_feedback = false;
    IncomingPacketFeedback(packet_feedback, msg.feedback_time);
    if (prev_detector_state == BandwidthUsage::kBwUnderusing &&
        self.active_delay_detector.State() == BandwidthUsage::kBwNormal) {
      recovered_from_overuse = true;
    }
    prev_detector_state = self.active_delay_detector.State();
  }

  if (delayed_feedback) {
    // TODO(bugs.webrtc.org/10125): Design a better mechanism to safe-guard
    // against building very large network queues.
    return Result();
  }
  self.rate_control.SetInApplicationLimitedRegion(in_alr);
  self.rate_control.SetNetworkStateEstimate(network_estimate);
  return MaybeUpdateEstimate(acked_bitrate, probe_bitrate,
                             std::move(network_estimate),
                             recovered_from_overuse, in_alr, msg.feedback_time);
}

fn IncomingPacketFeedback(&self /* DelayBasedBwe */,const PacketResult& packet_feedback,
                                           Timestamp at_time) {
  // Reset if the stream has timed out.
  if (self.last_seen_packet.IsInfinite() ||
      at_time - self.last_seen_packet > kStreamTimeOut) {
    self.video_inter_arrival_delta =
        std::make_unique<InterArrivalDelta>(kSendTimeGroupLength);
    self.audio_inter_arrival_delta =
        std::make_unique<InterArrivalDelta>(kSendTimeGroupLength);

    self.video_delay_detector.reset(
        new TrendlineEstimator(self.key_value_config, self.network_state_predictor));
    self.audio_delay_detector.reset(
        new TrendlineEstimator(self.key_value_config, self.network_state_predictor));
    self.active_delay_detector = self.video_delay_detector.get();
  }
  self.last_seen_packet = at_time;

  // As an alternative to ignoring small packets, we can separate audio and
  // video packets for overuse detection.
  DelayIncreaseDetectorInterface* delay_detector_for_packet =
      self.video_delay_detector.get();
  if (self.separate_audio.enabled) {
    if (packet_feedback.sent_packet.audio) {
      delay_detector_for_packet = self.audio_delay_detector.get();
      self.audio_packets_since_last_video += 1;
      if (self.audio_packets_since_last_video > self.separate_audio.packet_threshold &&
          packet_feedback.receive_time - self.last_video_packet_recv_time >
              self.separate_audio.time_threshold) {
        self.active_delay_detector = self.audio_delay_detector.get();
      }
    } else {
      self.audio_packets_since_last_video = 0;
      self.last_video_packet_recv_time =
          std::cmp::max(self.last_video_packet_recv_time, packet_feedback.receive_time);
      self.active_delay_detector = self.video_delay_detector.get();
    }
  }
  let packet_size: DataSize = packet_feedback.sent_packet.size;

  let send_delta: TimeDelta = TimeDelta::Zero();
  let recv_delta: TimeDelta = TimeDelta::Zero();
  let size_delta: isize = 0;

  InterArrivalDelta* inter_arrival_for_packet =
      (self.separate_audio.enabled && packet_feedback.sent_packet.audio)
          ? self.audio_inter_arrival_delta.get()
          : self.video_inter_arrival_delta.get();
  let calculated_deltas: bool = inter_arrival_for_packet->ComputeDeltas(
      packet_feedback.sent_packet.send_time, packet_feedback.receive_time,
      at_time, packet_size.bytes(), &send_delta, &recv_delta, &size_delta);

  delay_detector_for_packet->Update(recv_delta.ms<f64>(),
                                    send_delta.ms<f64>(),
                                    packet_feedback.sent_packet.send_time.ms(),
                                    packet_feedback.receive_time.ms(),
                                    packet_size.bytes(), calculated_deltas);
}

DataRate TriggerOveruse(&self /* DelayBasedBwe */,Timestamp at_time,
                                       Option<DataRate> link_capacity) {
  RateControlInput input(BandwidthUsage::kBwOverusing, link_capacity);
  return self.rate_control.Update(input, at_time);
}

DelayBasedBwe::Result MaybeUpdateEstimate(&self /* DelayBasedBwe */,
    Option<DataRate> acked_bitrate,
    Option<DataRate> probe_bitrate,
    Option<NetworkStateEstimate> /* state_estimate */,
    bool recovered_from_overuse,
    bool /* in_alr */,
    Timestamp at_time) {
  Result result;

  // Currently overusing the bandwidth.
  if (self.active_delay_detector.State() == BandwidthUsage::kBwOverusing) {
    if (acked_bitrate &&
        self.rate_control.TimeToReduceFurther(at_time, *acked_bitrate)) {
      result.updated =
          UpdateEstimate(at_time, acked_bitrate, &result.target_bitrate);
    } else if (!acked_bitrate && self.rate_control.ValidEstimate() &&
               self.rate_control.InitialTimeToReduceFurther(at_time)) {
      // Overusing before we have a measured acknowledged bitrate. Reduce send
      // rate by 50% every 200 ms.
      // TODO(tschumim): Improve this and/or the acknowledged bitrate estimator
      // so that we (almost) always have a bitrate estimate.
      self.rate_control.SetEstimate(self.rate_control.LatestEstimate() / 2, at_time);
      result.updated = true;
      result.probe = false;
      result.target_bitrate = self.rate_control.LatestEstimate();
    }
  } else {
    if (probe_bitrate) {
      result.probe = true;
      result.updated = true;
      self.rate_control.SetEstimate(*probe_bitrate, at_time);
      result.target_bitrate = self.rate_control.LatestEstimate();
    } else {
      result.updated =
          UpdateEstimate(at_time, acked_bitrate, &result.target_bitrate);
      result.recovered_from_overuse = recovered_from_overuse;
    }
  }
  let detector_state: BandwidthUsage = self.active_delay_detector.State();
  if ((result.updated && prev_bitrate_ != result.target_bitrate) ||
      detector_state != self.prev_state) {
    let bitrate: DataRate = result.updated ? result.target_bitrate : self.prev_bitrate;

    if (self.event_log) {
      self.event_log.Log(std::make_unique<RtcEventBweUpdateDelayBased>(
          bitrate.bps(), detector_state));
    }

    self.prev_bitrate = bitrate;
    self.prev_state = detector_state;
  }

  result.delay_detector_state = detector_state;
  return result;
}

bool UpdateEstimate(&self /* DelayBasedBwe */,Timestamp at_time,
                                   Option<DataRate> acked_bitrate,
                                   DataRate* target_rate) {
  const RateControlInput input(self.active_delay_detector.State(), acked_bitrate);
  *target_rate = self.rate_control.Update(input, at_time);
  return self.rate_control.ValidEstimate();
}

fn OnRttUpdate(&self /* DelayBasedBwe */,TimeDelta avg_rtt) {
  self.rate_control.SetRtt(avg_rtt);
}

bool LatestEstimate(&self /* DelayBasedBwe */,Vec<u32>* ssrcs,
                                   DataRate* bitrate) {
  // Currently accessed from both the process thread (see
  // ModuleRtpRtcpImpl::Process()) and the configuration thread (see
  // Call::GetStats()). Should in the future only be accessed from a single
  // thread.
  assert!(ssrcs);
  assert!(bitrate);
  if (!self.rate_control.ValidEstimate())
    return false;

  *ssrcs = {kFixedSsrc};
  *bitrate = self.rate_control.LatestEstimate();
  return true;
}

fn SetStartBitrate(&self /* DelayBasedBwe */,DataRate start_bitrate) {
  RTC_LOG(LS_INFO) << "BWE Setting start bitrate to: "
                   << ToString(start_bitrate);
  self.rate_control.SetStartBitrate(start_bitrate);
}

fn SetMinBitrate(&self /* DelayBasedBwe */,DataRate min_bitrate) {
  // Called from both the configuration thread and the network thread. Shouldn't
  // be called from the network thread in the future.
  self.rate_control.SetMinBitrate(min_bitrate);
}

TimeDelta GetExpectedBwePeriod(&self /* DelayBasedBwe */) {
  return self.rate_control.GetExpectedBandwidthPeriod();
}

}  // namespace webrtc
