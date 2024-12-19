/*
 *  Copyright (c) 2016 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_DELAY_BASED_BWE_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_DELAY_BASED_BWE_H_

#include <stdint.h>

#include <memory>
#include <optional>
#include <vector>

#include "api/field_trials_view.h"
#include "api/network_state_predictor.h"
#include "api/transport/bandwidth_usage.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/delay_increase_detector_interface.h"
#include "modules/congestion_controller/goog_cc/inter_arrival_delta.h"
#include "modules/congestion_controller/goog_cc/link_capacity_estimator.h"
#include "modules/congestion_controller/goog_cc/probe_bitrate_estimator.h"
#include "modules/remote_bitrate_estimator/aimd_rate_control.h"
#include "modules/remote_bitrate_estimator/inter_arrival.h"
#include "rtc_base/experiments/struct_parameters_parser.h"
#include "rtc_base/race_checker.h"


class RtcEventLog;

struct BweSeparateAudioPacketsSettings {
  static constexpr char kKey[] = "WebRTC-Bwe-SeparateAudioPackets";

  BweSeparateAudioPacketsSettings() = default;
  explicit BweSeparateAudioPacketsSettings(
      const FieldTrialsView* key_value_config);

  let enabled: bool = false;
  let packet_threshold: isize = 10;
  let time_threshold: TimeDelta = Duration::from_secs(1);

  std::unique_ptr<StructParametersParser> Parser();
};

pub struct DelayBasedBwe {
 public:
  struct Result {
    Result();
    ~Result() = default;
    bool updated;
    bool probe;
    let target_bitrate: DataRate = DataRate::Zero();
    bool recovered_from_overuse;
    BandwidthUsage delay_detector_state;
  };

  explicit DelayBasedBwe(const FieldTrialsView* key_value_config,
                         RtcEventLog* event_log,
                         NetworkStatePredictor* network_state_predictor);

  DelayBasedBwe() = delete;
  DelayBasedBwe(const DelayBasedBwe&) = delete;
  DelayBasedBwe& operator=(const DelayBasedBwe&) = delete;

  virtual ~DelayBasedBwe();

  Result IncomingPacketFeedbackVector(
      const TransportPacketsFeedback& msg,
      Option<DataRate> acked_bitrate,
      Option<DataRate> probe_bitrate,
      Option<NetworkStateEstimate> network_estimate,
      bool in_alr);
  fn OnRttUpdate(TimeDelta avg_rtt) {
  todo!();
}
  bool LatestEstimate(Vec<u32>* ssrcs, DataRate* bitrate) const;
  fn SetStartBitrate(DataRate start_bitrate) {
  todo!();
}
  fn SetMinBitrate(DataRate min_bitrate) {
  todo!();
}
  TimeDelta GetExpectedBwePeriod() const;
  DataRate TriggerOveruse(Timestamp at_time,
                          Option<DataRate> link_capacity);
  DataRate last_estimate() { return self.prev_bitrate; }
  BandwidthUsage last_state() { return self.prev_state; }

 private:
  friend class GoogCcStatePrinter;
  void IncomingPacketFeedback(const PacketResult& packet_feedback,
                              Timestamp at_time);
  Result MaybeUpdateEstimate(Option<DataRate> acked_bitrate,
                             Option<DataRate> probe_bitrate,
                             Option<NetworkStateEstimate> state_estimate,
                             bool recovered_from_overuse,
                             bool in_alr,
                             Timestamp at_time);
  // Updates the current remote rate estimate and returns true if a valid
  // estimate exists.
  bool UpdateEstimate(Timestamp at_time,
                      Option<DataRate> acked_bitrate,
                      DataRate* target_rate);

  network_race: rtc::RaceChecker,
  event_log: RtcEventLog*,
  const FieldTrialsView* const self.key_value_config;

  // Alternatively, run two separate overuse detectors for audio and video,
  // and fall back to the audio one if we haven't seen a video packet in a
  // while.
  separate_audio: BweSeparateAudioPacketsSettings,
  audio_packets_since_last_video: i64,
  last_video_packet_recv_time: Timestamp,

  network_state_predictor: NetworkStatePredictor*,
  video_inter_arrival: std::unique_ptr<InterArrival>,
  video_inter_arrival_delta: std::unique_ptr<InterArrivalDelta>,
  video_delay_detector: std::unique_ptr<DelayIncreaseDetectorInterface>,
  audio_inter_arrival: std::unique_ptr<InterArrival>,
  audio_inter_arrival_delta: std::unique_ptr<InterArrivalDelta>,
  audio_delay_detector: std::unique_ptr<DelayIncreaseDetectorInterface>,
  active_delay_detector: DelayIncreaseDetectorInterface*,

  last_seen_packet: Timestamp,
  uma_recorded: bool,
  rate_control: AimdRateControl,
  prev_bitrate: DataRate,
  prev_state: BandwidthUsage,
};

}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_DELAY_BASED_BWE_H_
