/*
 *  Copyright 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */
#ifndef MODULES_CONGESTION_CONTROLLER_GOOG_CC_TEST_GOOG_CC_PRINTER_H_
#define MODULES_CONGESTION_CONTROLLER_GOOG_CC_TEST_GOOG_CC_PRINTER_H_

#include <deque>
#include <memory>
#include <string>

#include "api/rtc_event_log_output.h"
#include "api/transport/goog_cc_factory.h"
#include "api/transport/network_control.h"
#include "api/transport/network_types.h"
#include "api/units/data_size.h"
#include "api/units/timestamp.h"
#include "modules/congestion_controller/goog_cc/goog_cc_network_control.h"



pub struct FieldLogger {
 public:
  virtual ~FieldLogger() = default;
  virtual const std::string& name() const = 0;
  virtual void WriteValue(RtcEventLogOutput* out) = 0;
};

pub struct GoogCcStatePrinter {
 public:
  GoogCcStatePrinter();
  GoogCcStatePrinter(const GoogCcStatePrinter&) = delete;
  GoogCcStatePrinter& operator=(const GoogCcStatePrinter&) = delete;
  ~GoogCcStatePrinter();

  fn PrintHeaders(RtcEventLogOutput* log) {
  todo!();
}
  void PrintState(RtcEventLogOutput* log,
                  GoogCcNetworkController* controller,
                  Timestamp at_time);

 private:
  VecDeque<FieldLogger*> CreateLoggers();
  loggers: VecDeque<std::unique_ptr<FieldLogger>>,

  GoogCcNetworkController* self.controller = nullptr;
  target: TargetTransferRate,
  pacing: PacerConfig,
  DataSize self.congestion_window = DataSize::PlusInfinity();
  est: NetworkStateEstimate,
};

impl GoogCcNetworkControllerFactory for GoogCcDebugFactory {

}

pub struct GoogCcDebugFactory {
 public:
  GoogCcDebugFactory();
  explicit GoogCcDebugFactory(GoogCcFactoryConfig config);
  std::unique_ptr<NetworkControllerInterface> Create(
      NetworkControllerConfig config) override;

  fn PrintState(Timestamp at_time) {
  todo!();
}

  fn AttachWriter(std::unique_ptr<RtcEventLogOutput> log_writer) {
  todo!();
}

 private:
  printer: GoogCcStatePrinter,
  GoogCcNetworkController* self.controller = nullptr;
  log_writer: std::unique_ptr<RtcEventLogOutput>,
};
}  // namespace webrtc

#endif  // MODULES_CONGESTION_CONTROLLER_GOOG_CC_TEST_GOOG_CC_PRINTER_H_
