/*
 *  Copyright 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */
#include "modules/congestion_controller/goog_cc/test/goog_cc_printer.h"

#include <math.h>

#include <deque>
#include <memory>
#include <optional>
#include <string>
#include <utility>

#include "absl/strings/string_view.h"
#include "api/rtc_event_log_output.h"
#include "api/transport/goog_cc_factory.h"
#include "api/transport/network_control.h"
#include "api/transport/network_types.h"
#include "api/units/data_rate.h"
#include "api/units/data_size.h"
#include "api/units/time_delta.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/alr_detector.h"
#include "modules/congestion_controller/goog_cc/delay_based_bwe.h"
#include "modules/congestion_controller/goog_cc/goog_cc_network_control.h"
#include "modules/congestion_controller/goog_cc/trendline_estimator.h"
#include "modules/remote_bitrate_estimator/aimd_rate_control.h"
#include "rtc_base/checks.h"
#include "test/logging/log_writer.h"


namespace {
fn WriteTypedValue(RtcEventLogOutput* out, isize value) {
  LogWriteFormat(out, "%i", value);
}
fn WriteTypedValue(RtcEventLogOutput* out, f64 value) {
  LogWriteFormat(out, "%.6f", value);
}
fn WriteTypedValue(RtcEventLogOutput* out, Option<DataRate> value) {
  LogWriteFormat(out, "%.0f", value ? value.bytes_per_sec<f64>() : NAN);
}
fn WriteTypedValue(RtcEventLogOutput* out, Option<DataSize> value) {
  LogWriteFormat(out, "%.0f", value ? value.bytes<f64>() : NAN);
}
fn WriteTypedValue(RtcEventLogOutput* out, Option<TimeDelta> value) {
  LogWriteFormat(out, "%.3f", value ? value.seconds<f64>() : NAN);
}
fn WriteTypedValue(RtcEventLogOutput* out, Option<Timestamp> value) {
  LogWriteFormat(out, "%.3f", value ? value.seconds<f64>() : NAN);
}

template <typename F>
impl FieldLogger for TypedFieldLogger {

}

pub struct TypedFieldLogger {
 public:
  TypedFieldLogger(absl::string_view name, F&& getter)
      : name_(name), getter_(std::forward<F>(getter)) {}
  const std::string& name() const override { return self.name; }
  void WriteValue(RtcEventLogOutput* out) override {
    WriteTypedValue(out, getter_());
  }

 private:
  name: std::string,
  getter: F,
};

template <typename F>
fn Log(absl::string_view name, F&& getter) -> FieldLogger* {
  return new TypedFieldLogger<F>(name, std::forward<F>(getter));
}

}  // namespace
GoogCcStatePrinter::GoogCcStatePrinter() {
  for (auto* logger : CreateLoggers()) {
    self.loggers.push_back(logger);
  }
}

VecDeque<FieldLogger*> CreateLoggers(&self /* GoogCcStatePrinter */) {
let stable_estimate = [this] {
    return DataRate::KilobitsPerSec(
        self.controller.self.delay_based_bwe.self.rate_control.link_capacity_
            .self.estimate_kbps.unwrap_or(-INFINITY));
  };
let rate_control_state = [this] {
    return static_cast<isize>(
        self.controller.self.delay_based_bwe.self.rate_control.self.rate_control_state);
  };
let trend = [this] {
    return reinterpret_cast<TrendlineEstimator*>(
        self.controller.self.delay_based_bwe.self.active_delay_detector);
  };
let acknowledged_rate = [this] {
    return self.controller.self.acknowledged_bitrate_estimator.bitrate();
  };
let loss_cont = [&] {
    return &self.controller.bandwidth_estimation_
                .self.loss_based_bandwidth_estimator_v1;
  };
  VecDeque<FieldLogger*> loggers({
      Log("time", [this] { return self.target.at_time; }),
      Log("rtt", [this] { return self.target.network_estimate.round_trip_time; }),
      Log("target", [this] { return self.target.target_rate; }),
      Log("stable_target", [this] { return self.target.stable_target_rate; }),
      Log("pacing", [this] { return self.pacing.data_rate(); }),
      Log("padding", [this] { return self.pacing.pad_rate(); }),
      Log("window", [this] { return self.congestion_window; }),
      Log("rate_control_state", [=] { return rate_control_state(); }),
      Log("stable_estimate", [=] { return stable_estimate(); }),
      Log("trendline", [=] { return trend().self.prev_trend; }),
      Log("trendline_modified_offset",
          [=] { return trend().self.prev_modified_trend; }),
      Log("trendline_offset_threshold", [=] { return trend().self.threshold; }),
      Log("acknowledged_rate", [=] { return acknowledged_rate(); }),
      Log("est_capacity", [this] { return self.est.link_capacity; }),
      Log("est_capacity_dev", [this] { return self.est.link_capacity_std_dev; }),
      Log("est_capacity_min", [this] { return self.est.link_capacity_min; }),
      Log("est_cross_traffic", [this] { return self.est.cross_traffic_ratio; }),
      Log("est_cross_delay", [this] { return self.est.cross_delay_rate; }),
      Log("est_spike_delay", [this] { return self.est.spike_delay_rate; }),
      Log("est_pre_buffer", [this] { return self.est.pre_link_buffer_delay; }),
      Log("est_post_buffer", [this] { return self.est.post_link_buffer_delay; }),
      Log("est_propagation", [this] { return self.est.propagation_delay; }),
      Log("loss_ratio", [=] { return loss_cont().self.last_loss_ratio; }),
      Log("loss_average", [=] { return loss_cont().self.average_loss; }),
      Log("loss_average_max", [=] { return loss_cont().self.average_loss_max; }),
      Log("loss_thres_inc",
          [=] { return loss_cont().loss_increase_threshold(); }),
      Log("loss_thres_dec",
          [=] { return loss_cont().loss_decrease_threshold(); }),
      Log("loss_dec_rate", [=] { return loss_cont().decreased_bitrate(); }),
      Log("loss_based_rate", [=] { return loss_cont().self.loss_based_bitrate; }),
      Log("loss_ack_rate",
          [=] { return loss_cont().self.acknowledged_bitrate_max; }),
      Log("data_window", [this] { return self.controller.self.current_data_window; }),
      Log("pushback_target",
          [this] { return self.controller.self.last_pushback_target_rate; }),
  });
  return loggers;
}
GoogCcStatePrinter::~GoogCcStatePrinter() = default;

fn PrintHeaders(&self /* GoogCcStatePrinter */,RtcEventLogOutput* log) {
  let ix: isize = 0;
  for logger in &self.loggers {
    if (ix++)
      log.Write(" ");
    log.Write(logger.name());
  }
  log.Write("\n");
  log.Flush();
}

fn PrintState(&self /* GoogCcStatePrinter */,RtcEventLogOutput* log,
                                    GoogCcNetworkController* controller,
                                    Timestamp at_time) {
  self.controller = controller;
let state_update = self.controller.GetNetworkState(at_time);
  self.target = state_update.target_rate.value();
  self.pacing = state_update.pacer_config.value();
  if (state_update.congestion_window)
    self.congestion_window = *state_update.congestion_window;
  if (self.controller.self.network_estimator) {
    self.est = self.controller.self.network_estimator.GetCurrentEstimate().unwrap_or(
        NetworkStateEstimate());
  }

  let ix: isize = 0;
  for logger in &self.loggers {
    if (ix++)
      log.Write(" ");
    logger.WriteValue(log);
  }

  log.Write("\n");
  log.Flush();
}

GoogCcDebugFactory::GoogCcDebugFactory()
    : GoogCcDebugFactory(GoogCcFactoryConfig()) {}

GoogCcDebugFactory::GoogCcDebugFactory(GoogCcFactoryConfig config)
    : GoogCcNetworkControllerFactory(std::move(config)) {}

std::unique_ptr<NetworkControllerInterface> Create(&self /* GoogCcDebugFactory */,
    NetworkControllerConfig config) {
  RTC_CHECK(self.controller == nullptr);
let controller = GoogCcNetworkControllerFactory::Create(config);
  self.controller = static_cast<GoogCcNetworkController*>(controller.get());
  return controller;
}

fn PrintState(&self /* GoogCcDebugFactory */,const Timestamp at_time) {
  if (self.controller && self.log_writer) {
    self.printer.PrintState(self.log_writer.get(), self.controller, at_time);
  }
}

fn AttachWriter(&self /* GoogCcDebugFactory */,
    std::unique_ptr<RtcEventLogOutput> log_writer) {
  if (log_writer) {
    self.log_writer = std::move(log_writer);
    self.printer.PrintHeaders(self.log_writer.get());
  }
}

}  // namespace webrtc
