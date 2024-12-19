/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

 // DataRate is a class that represents a given data rate. This can be used to
 // represent bandwidth, encoding bitrate, etc. The internal storage is bits per
 // second (bps).

use std::{fmt};
use std::ops::*;

use super::{DataSize, Frequency, TimeDelta};

super::relative_unit!(DataRate);

impl DataRate {
  const ONE_SIDED: bool = true;

   pub const fn BitsPerSec(value: i64) -> Self {
    Self::FromValue(value)
   }

   pub const fn BitsPerSecFloat(value: f64) -> Self {
    Self::FromValueFloat(value)
   }

   pub const fn BytesPerSec(value: i64) -> Self {
    Self::FromFraction(8, value)
   }

   pub const fn BytesPerSecFloat(value: f64) -> Self {
    Self::FromFractionFloat(8.0, value)
   }

   pub const fn KilobitsPerSec(value: i64) -> Self {
     Self::FromFraction(1000, value)
   }

   pub const fn KilobitsPerSecFloat(value: f64) -> Self {
     Self::FromFractionFloat(1000.0, value)
   }

   pub const fn Infinity() -> Self { Self::PlusInfinity() }

   pub const fn bps(&self) -> i64 {
    self.ToValue()
   }

   pub const fn bps_float(&self) -> f64 {
    self.ToValueFloat()
   }

   pub const fn bytes_per_sec(&self) -> i64 { self.ToFraction(8) }

   pub const fn bytes_per_sec_float(&self) -> f64 {
    self.ToFractionFloat(8.0)
   }

   pub const fn kbps(&self) -> i64 {
    self.ToFraction(1000)
}

    pub const fn kbps_float(&self) -> f64 {
      self.ToFractionFloat(1000.0)
    }

   pub const fn bps_or(&self, fallback_value: i64) ->i64{
     self.ToFractionOr(1, fallback_value)
   }

   pub const fn kbps_or(&self, fallback_value: i64) -> i64 {
     self.ToFractionOr(1000, fallback_value)
   }

   pub const fn MillibytePerSec(&self) -> i64 {
    const MaxBeforeConversion: i64 =
        i64::MAX / (1000 / 8);
    assert!(self.bps() < MaxBeforeConversion, "rate is too large to be expressed in microbytes per second");
    self.bps() * (1000 / 8)
   }
 }

 impl Div<TimeDelta> for DataSize {
   type Output = DataRate;

   fn div(self, duration: TimeDelta) -> Self::Output {
    DataRate::BitsPerSec(self.Microbits() / duration.us())
   }
 }

 impl Div<DataRate> for DataSize {
   type Output = TimeDelta;

   fn div(self, rate: DataRate) -> Self::Output {
    TimeDelta::Micros(self.Microbits() / rate.bps())
   }
 }

 impl Mul<TimeDelta> for DataRate{
   type Output = DataSize;

   fn mul(self, duration: TimeDelta) -> Self::Output {
    let microbits: i64 = self.bps() * duration.us();
    DataSize::Bytes((microbits + 4000000) / 8000000)
   }
 }

 impl Mul<DataRate> for TimeDelta{
   type Output = DataSize;

   fn mul(self, rate: DataRate) -> Self::Output {
    rate * self
   }
 }

 impl Div<Frequency> for DataRate {
    type Output = DataSize;

    fn div(self, frequency: Frequency) -> Self::Output {
    let millihertz: i64 = frequency.millihertz();
    // Note that the value is truncated here reather than rounded, potentially
    // introducing an error of .5 bytes if rounding were expected.
    DataSize::Bytes(self.MillibytePerSec() / millihertz)
    }
 }

 impl Div<DataSize> for DataRate {
    type Output = Frequency;

    fn div(self, size: DataSize) -> Self::Output {
        Frequency::MilliHertz(self.MillibytePerSec() / size.bytes())
    }
 }

 impl Mul<Frequency> for DataSize {
    type Output = DataRate;

    fn mul(self, frequency: Frequency) -> Self::Output {
        assert!(frequency.IsZero() ||
                    self.bytes() <= i64::MAX / 8 /
                                        frequency.millihertz());
        let millibits_per_second: i64 =
            self.bytes() * 8 * frequency.millihertz();
        DataRate::BitsPerSec((millibits_per_second + 500) / 1000)
    }
 }

 impl Mul<DataSize> for Frequency {
    type Output = DataRate;

    fn mul(self, size: DataSize) -> Self::Output {
        size * self
    }
 }

 impl fmt::Debug for DataRate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.IsPlusInfinity() {
            write!(f, "+inf bps")
        } else if self.IsMinusInfinity() {
            write!(f, "-inf bps")
        } else if self.bps() == 0 || self.bps() % 1000 != 0 {
            write!(f, "{} bps", self.bps())
        } else {
            write!(f, "{} kbps", self.kbps())
        }
    }
}

 #[cfg(test)]
 mod test {
    use super::*;

    #[test]
    fn CompilesWithChecksAndLogs() {
    let a: DataRate = DataRate::KilobitsPerSec(300);
    let b: DataRate = DataRate::KilobitsPerSec(210);
    assert!(a> b);
    println!("{:?}", a);
 }

 #[test]
 fn ConstExpr() {
   const Value: i64 = 12345;
   const DataRateZero: DataRate = DataRate::Zero();
   const DataRateInf: DataRate = DataRate::Infinity();
   assert_eq!(DataRate::default(), DataRateZero);
   assert!(DataRateZero.IsZero());
   assert!(DataRateInf.IsInfinite());
   assert!(DataRateInf.bps_or(-1) == -1);
   assert!(DataRateInf > DataRateZero);

   const DataRateBps: DataRate = DataRate::BitsPerSec(Value);
   const DataRateKbps: DataRate = DataRate::KilobitsPerSec(Value);
   assert!(DataRateBps.bps() == Value);
   assert!(DataRateBps.bps_or(0) == Value);
   assert!(DataRateKbps.kbps_or(0) == Value);
 }

#[test]
fn GetBackSameValues() {
   const Value: i64 = 123 * 8;
   assert_eq!(DataRate::BitsPerSec(Value).bps(), Value);
   assert_eq!(DataRate::KilobitsPerSec(Value).kbps(), Value);
 }

#[test]
fn GetDifferentPrefix() {
   const Value: i64 = 123 * 8000;
   assert_eq!(DataRate::BitsPerSec(Value).kbps(), Value / 1000);
 }

#[test]
fn IdentityChecks() {
   const Value: i64 = 3000;
   assert!(DataRate::Zero().IsZero());
   assert!(!DataRate::BitsPerSec(Value).IsZero());

   assert!(DataRate::Infinity().IsInfinite());
   assert!(!DataRate::Zero().IsInfinite());
   assert!(!DataRate::BitsPerSec(Value).IsInfinite());

   assert!(!DataRate::Infinity().IsFinite());
   assert!(DataRate::BitsPerSec(Value).IsFinite());
   assert!(DataRate::Zero().IsFinite());
 }

#[test]
fn ComparisonOperators() {
   const Small: i64 = 450;
   const Large: i64 = 451;
   const small: DataRate = DataRate::BitsPerSec(Small);
   const large: DataRate = DataRate::BitsPerSec(Large);

   assert_eq!(DataRate::Zero(), DataRate::BitsPerSec(0));
   assert_eq!(DataRate::Infinity(), DataRate::Infinity());
   assert_eq!(small, small);
   assert!(small <= small);
   assert!(small >= small);
   assert!(small !=large);
   assert!(small <= large);
   assert!(small < large);
   assert!(large >= small);
   assert!(large > small);
   assert!(DataRate::Zero() < small);
   assert!(DataRate::Infinity() > large);
 }

#[test]
fn ConvertsToAndFromDouble() {
   const Value: i64 = 128;
   const kDoubleValue: f64 = Value as f64;
   const kDoubleKbps: f64 = Value as f64 * 1e-3;
   const kFloatKbps: f32 = kDoubleKbps as f32;

   assert_eq!(DataRate::BitsPerSec(Value).bps_float(), kDoubleValue);
   assert_eq!(DataRate::BitsPerSec(Value).kbps_float(), kDoubleKbps);
   assert_eq!(DataRate::BitsPerSec(Value).kbps_float() as f32, kFloatKbps);
   assert_eq!(DataRate::BitsPerSecFloat(kDoubleValue).bps(), Value);
   assert_eq!(DataRate::KilobitsPerSecFloat(kDoubleKbps).bps(), Value);

   const kInfinity: f64 = f64::INFINITY;
   assert_eq!(DataRate::Infinity().bps_float(), kInfinity);
   assert!(DataRate::BitsPerSecFloat(kInfinity).IsInfinite());
   assert!(DataRate::KilobitsPerSecFloat(kInfinity).IsInfinite());
 }
#[test]
fn Clamping() {
   const upper :DataRate = DataRate::KilobitsPerSec(800);
   const lower: DataRate = DataRate::KilobitsPerSec(100);
   const under :DataRate = DataRate::KilobitsPerSec(100);
   const inside: DataRate = DataRate::KilobitsPerSec(500);
   const over: DataRate = DataRate::KilobitsPerSec(1000);
   assert_eq!(under.Clamped(lower, upper), lower);
   assert_eq!(inside.Clamped(lower, upper), inside);
   assert_eq!(over.Clamped(lower, upper), upper);

   let mut mutable_rate: DataRate = lower;
   mutable_rate.Clamp(lower, upper);
   assert_eq!(mutable_rate, lower);
   mutable_rate = inside;
   mutable_rate.Clamp(lower, upper);
   assert_eq!(mutable_rate, inside);
   mutable_rate = over;
   mutable_rate.Clamp(lower, upper);
   assert_eq!(mutable_rate, upper);
 }

#[test]
fn MathOperations() {
   const ValueA: i64 = 450;
   const ValueB: i64 = 267;
   const rate_a: DataRate = DataRate::BitsPerSec(ValueA);
   const rate_b: DataRate = DataRate::BitsPerSec(ValueB);
   const kInt32Value: i32 = 123;
   const kFloatValue: f64 = 123.0;

   assert_eq!((rate_a + rate_b).bps(), ValueA + ValueB);
   assert_eq!((rate_a - rate_b).bps(), ValueA - ValueB);

   assert_eq!((rate_a * ValueB).bps(), ValueA * ValueB);
   assert_eq!((rate_a * kInt32Value).bps(), ValueA * kInt32Value as i64);
   assert_eq!((rate_a * kFloatValue).bps(), ValueA * kFloatValue as i64);

   assert_eq!(rate_a / rate_b, ValueA as f64 / ValueB as f64);

   assert_eq!((rate_a / 10).bps(), ValueA / 10);
   assert!(((rate_a / 0.5).bps() - ValueA * 2).abs() <= 1);

   let mut mutable_rate: DataRate = DataRate::BitsPerSec(ValueA);
   mutable_rate += rate_b;
   assert_eq!(mutable_rate.bps(), ValueA + ValueB);
   mutable_rate -= rate_a;
   assert_eq!(mutable_rate.bps(), ValueB);
 }

#[test]
fn DataRateAndDataSizeAndTimeDelta() {
   const Seconds: i64 = 5;
   const BitsPerSecond: i64 = 440;
   const Bytes: i64 = 44000;
   const delta_a: TimeDelta = TimeDelta::Seconds(Seconds);
   const rate_b: DataRate = DataRate::BitsPerSec(BitsPerSecond);
   const size_c: DataSize = DataSize::Bytes(Bytes);
   assert_eq!((delta_a * rate_b).bytes(), Seconds * BitsPerSecond / 8);
   assert_eq!((rate_b * delta_a).bytes(), Seconds * BitsPerSecond / 8);
   assert_eq!((size_c / delta_a).bps(), Bytes * 8 / Seconds);
   assert_eq!((size_c / rate_b).seconds(), Bytes * 8 / BitsPerSecond);
 }

#[test]
fn DataRateAndDataSizeAndFrequency() {
   const Hertz: i64 = 30;
   const BitsPerSecond: i64 = 96000;
   const Bytes: i64 = 1200;
   const freq_a: Frequency = Frequency::Hertz(Hertz);
   const rate_b: DataRate = DataRate::BitsPerSec(BitsPerSecond);
   const size_c: DataSize = DataSize::Bytes(Bytes);
   assert_eq!((freq_a * size_c).bps(), Hertz * Bytes * 8);
   assert_eq!((size_c * freq_a).bps(), Hertz * Bytes * 8);
   assert_eq!((rate_b / size_c).hertz(), BitsPerSecond / Bytes / 8);
   assert_eq!((rate_b / freq_a).bytes(), BitsPerSecond / Hertz / 8);
 }

   const JustSmallEnoughForDivision: i64 = i64::MAX / 8000000;
   const ToolargeForDivision : DataSize = DataSize::Bytes(JustSmallEnoughForDivision + 1);

#[test]
fn DivisionFailsOnLargeSize() {
   // Note that the failure is expected since the current implementation  is
   // implementated in a way that does not support division of large sizes. If
   // the implementation is changed, this test can safely be removed.
   const large_size: DataSize = DataSize::Bytes(JustSmallEnoughForDivision);
   const data_rate: DataRate = DataRate::KilobitsPerSec(100);
   const time_delta: TimeDelta = TimeDelta::Millis(100);
   assert!((large_size / data_rate).IsFinite());
   assert!((large_size / time_delta).IsFinite());
}

#[test]
#[should_panic]
fn DivisionFailsOnLargeSize1() {
   const data_rate: DataRate = DataRate::KilobitsPerSec(100);
   ToolargeForDivision / data_rate;
 }
#[test]
#[should_panic]
fn DivisionFailsOnLargeSize2() {
   const time_delta: TimeDelta = TimeDelta::Millis(100);
   ToolargeForDivision / time_delta;
}
 }
