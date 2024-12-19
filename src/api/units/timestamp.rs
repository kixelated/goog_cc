/*
 *  Copyright (c) 2018 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::fmt;
use std::ops::*;

use super::TimeDelta;

 // Timestamp represents the time that has passed since some unspecified epoch.
 // The epoch is assumed to be before any represented timestamps, this means that
 // negative values are not valid. The most notable feature is that the
 // difference of two Timestamps results in a TimeDelta.
 super::unit_base!(Timestamp);

 impl Timestamp {
  const ONE_SIDED: bool = false;

    pub const fn Seconds(value: i64) -> Self {
      Self::FromFraction(1_000_000, value)
    }

    pub const fn SecondsFloat(value: f64) -> Self {
      Self::FromFractionFloat(1_000_000.0, value)
    }

    pub const fn Millis(value: i64) -> Self {
      Self::FromFraction(1_000, value)
    }

    pub const fn MillisFloat(value: f64) -> Self {
      Self::FromFractionFloat(1_000.0, value)
    }

    pub const fn Micros(value: i64) -> Self {
      Self::FromValue(value)
    }

    pub const fn MicrosFloat(value: f64) -> Self {
      Self::FromValueFloat(value)
    }

    pub const fn seconds(&self) -> i64 {
      self.ToFraction(1_000_000)
    }

    pub const fn seconds_float(&self) -> f64 {
      self.ToFractionFloat(1_000_000.0)
    }

    pub const fn ms(&self) -> i64{
      self.ToFraction(1_000)
    }

    pub const fn ms_float(&self) -> f64 {
      self.ToFractionFloat(1_000.0)
    }

    pub const fn us(&self) -> i64 {
      self.ToValue()
    }

    pub const fn us_float(&self) -> f64 {
      self.ToValueFloat()
    }

    pub const fn seconds_or(&self, fallback_value: i64) -> i64{
      self.ToFractionOr(1_000_000, fallback_value)
    }

    pub const fn ms_or(&self, fallback_value: i64) -> i64 {
      self.ToFractionOr(1_000, fallback_value)
    }

    pub const fn us_or(&self, fallback_value: i64) -> i64 {
      self.ToValueOr(fallback_value)
    }
}

impl Add<TimeDelta> for Timestamp {
    type Output = Self;

    fn add(self, delta: TimeDelta) -> Self {
        if self.IsPlusInfinity() || delta.IsPlusInfinity() {
            assert!(!self.IsMinusInfinity());
            assert!(!delta.IsMinusInfinity());
            return Self::PlusInfinity();
        } else if self.IsMinusInfinity() || delta.IsMinusInfinity() {
            assert!(!self.IsPlusInfinity());
            assert!(!delta.IsPlusInfinity());
            return Self::MinusInfinity();
        }
        Timestamp::Micros(self.us() + delta.us())
    }
}

impl Sub<TimeDelta> for Timestamp {
    type Output = Self;

    fn sub(self, delta: TimeDelta) -> Self {
        if self.IsPlusInfinity() || delta.IsMinusInfinity() {
            assert!(!self.IsMinusInfinity());
            assert!(!delta.IsPlusInfinity());
            return Self::PlusInfinity();
        } else if self.IsMinusInfinity() || delta.IsPlusInfinity() {
            assert!(!self.IsPlusInfinity());
            assert!(!delta.IsMinusInfinity());
            return Self::MinusInfinity();
        }
        Timestamp::Micros(self.us() - delta.us())
    }
}

impl Sub for Timestamp {
    type Output = TimeDelta;

    fn sub(self, other: Self) -> TimeDelta {
        if self.IsPlusInfinity() || other.IsMinusInfinity() {
            assert!(!self.IsMinusInfinity());
            assert!(!other.IsPlusInfinity());
            return TimeDelta::PlusInfinity();
        } else if self.IsMinusInfinity() || other.IsPlusInfinity() {
            assert!(!self.IsPlusInfinity());
            assert!(!other.IsMinusInfinity());
            return TimeDelta::MinusInfinity();
        }
        TimeDelta::Micros(self.us() - other.us())
    }
}

impl AddAssign<TimeDelta> for Timestamp {
    fn add_assign(&mut self, delta: TimeDelta) {
        *self = *self + delta;
    }
}

impl SubAssign<TimeDelta> for Timestamp {
    fn sub_assign(&mut self, delta: TimeDelta) {
        *self = *self - delta;
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.IsPlusInfinity() {
            write!(f, "+inf ms")
        } else if self.IsMinusInfinity() {
            write!(f, "-inf ms")
        } else if self.us() == 0 || (self.us() % 1000) != 0 {
                write!(f, "{} us", self.us())
            } else if self.ms() % 1000 != 0 {
                write!(f, "{} ms", self.ms())
            } else {
                write!(f, "{} s", self.seconds())
            }
    }
}

#[cfg(test)]
mod test {
    use super::*;

#[test]
fn ConstExpr() {
  const Value: i64 =  12345;
  const TimestampInf: Timestamp =  Timestamp::PlusInfinity();
  assert!(TimestampInf.IsInfinite());
  assert!(TimestampInf.ms_or(-1) == -1);

  const TimestampSeconds: Timestamp =  Timestamp::Seconds(Value);
  const TimestampMs: Timestamp =  Timestamp::Millis(Value);
  const TimestampUs: Timestamp =  Timestamp::Micros(Value);

  assert!(TimestampSeconds.seconds_or(0) == Value);
  assert!(TimestampMs.ms_or(0) == Value);
  assert!(TimestampUs.us_or(0) == Value);

  assert!(TimestampMs > TimestampUs);

  assert_eq!(TimestampSeconds.seconds(), Value);
  assert_eq!(TimestampMs.ms(), Value);
  assert_eq!(TimestampUs.us(), Value);
}

#[test]
fn GetBackSameValues() {
  const Value: i64 =  499;
  assert_eq!(Timestamp::Millis(Value).ms(), Value);
  assert_eq!(Timestamp::Micros(Value).us(), Value);
  assert_eq!(Timestamp::Seconds(Value).seconds(), Value);
}

#[test]
fn GetDifferentPrefix() {
  const Value: i64 =  3000000;
  assert_eq!(Timestamp::Micros(Value).seconds(), Value / 1000000);
  assert_eq!(Timestamp::Millis(Value).seconds(), Value / 1000);
  assert_eq!(Timestamp::Micros(Value).ms(), Value / 1000);

  assert_eq!(Timestamp::Millis(Value).us(), Value * 1000);
  assert_eq!(Timestamp::Seconds(Value).ms(), Value * 1000);
  assert_eq!(Timestamp::Seconds(Value).us(), Value * 1000000);
}

#[test]
fn IdentityChecks() {
  const Value: i64 =  3000;

  assert!(Timestamp::PlusInfinity().IsInfinite());
  assert!(Timestamp::MinusInfinity().IsInfinite());
  assert!(!Timestamp::Millis(Value).IsInfinite());

  assert!(!Timestamp::PlusInfinity().IsFinite());
  assert!(!Timestamp::MinusInfinity().IsFinite());
  assert!(Timestamp::Millis(Value).IsFinite());

  assert!(Timestamp::PlusInfinity().IsPlusInfinity());
  assert!(!Timestamp::MinusInfinity().IsPlusInfinity());

  assert!(Timestamp::MinusInfinity().IsMinusInfinity());
  assert!(!Timestamp::PlusInfinity().IsMinusInfinity());
}

#[test]
fn ComparisonOperators() {
  const Small: i64 =  450;
  const Large: i64 =  451;

  assert_eq!(Timestamp::PlusInfinity(), Timestamp::PlusInfinity());
  assert!(Timestamp::PlusInfinity() >= Timestamp::PlusInfinity());
  assert!(Timestamp::PlusInfinity() > Timestamp::Millis(Large));
  assert_eq!(Timestamp::Millis(Small), Timestamp::Millis(Small));
  assert!(Timestamp::Millis(Small) <= Timestamp::Millis(Small));
  assert!(Timestamp::Millis(Small) >= Timestamp::Millis(Small));
  assert!(Timestamp::Millis(Small) != Timestamp::Millis(Large));
  assert!(Timestamp::Millis(Small) <= Timestamp::Millis(Large));
  assert!(Timestamp::Millis(Small) < Timestamp::Millis(Large));
  assert!(Timestamp::Millis(Large) >= Timestamp::Millis(Small));
  assert!(Timestamp::Millis(Large) > Timestamp::Millis(Small));
}

#[test]
fn CanBeInititializedFromLargeInt() {
  const MaxInt: i32 =  i32::MAX;
  assert_eq!(Timestamp::Seconds(MaxInt as _).us(),
            MaxInt as i64 * 1000000);
  assert_eq!(Timestamp::Millis(MaxInt as _).us(),
            MaxInt as i64 * 1000);
}

#[test]
fn ConvertsToAndFromDouble() {
  const Micros: i64 =  17017;
  const MicrosDouble: f64 =  Micros as f64;
  const MillisDouble: f64 =  Micros as f64 * 1e-3;
  const SecondsDouble: f64 =  MillisDouble * 1e-3;

  assert_eq!(Timestamp::Micros(Micros).seconds_float(), SecondsDouble);
  assert_eq!(Timestamp::SecondsFloat(SecondsDouble).us(), Micros);

  assert_eq!(Timestamp::Micros(Micros).ms_float(), MillisDouble);
  assert_eq!(Timestamp::MillisFloat(MillisDouble).us(), Micros);

  assert_eq!(Timestamp::Micros(Micros).us_float(), MicrosDouble);
  assert_eq!(Timestamp::MicrosFloat(MicrosDouble).us(), Micros);

  const PlusInfinity: f64 =  f64::INFINITY;
  const MinusInfinity: f64 =  -PlusInfinity;

  assert_eq!(Timestamp::PlusInfinity().seconds_float(), PlusInfinity);
  assert_eq!(Timestamp::MinusInfinity().seconds_float(), MinusInfinity);
  assert_eq!(Timestamp::PlusInfinity().ms_float(), PlusInfinity);
  assert_eq!(Timestamp::MinusInfinity().ms_float(), MinusInfinity);
  assert_eq!(Timestamp::PlusInfinity().us_float(), PlusInfinity);
  assert_eq!(Timestamp::MinusInfinity().us_float(), MinusInfinity);

  assert!(Timestamp::SecondsFloat(PlusInfinity).IsPlusInfinity());
  assert!(Timestamp::SecondsFloat(MinusInfinity).IsMinusInfinity());
  assert!(Timestamp::MillisFloat(PlusInfinity).IsPlusInfinity());
  assert!(Timestamp::MillisFloat(MinusInfinity).IsMinusInfinity());
  assert!(Timestamp::MicrosFloat(PlusInfinity).IsPlusInfinity());
  assert!(Timestamp::MicrosFloat(MinusInfinity).IsMinusInfinity());
}

#[test]
fn TimestampAndTimeDeltaMath() {
  const ValueA: i64 =  267;
  const ValueB: i64 =  450;
  const time_a: Timestamp = Timestamp::Millis(ValueA);
  const time_b: Timestamp = Timestamp::Millis(ValueB);
  const delta_a: TimeDelta = TimeDelta::Millis(ValueA);
  const delta_b: TimeDelta = TimeDelta::Millis(ValueB);

  assert_eq!((time_a - time_b), TimeDelta::Millis(ValueA - ValueB));
  assert_eq!((time_b - delta_a), Timestamp::Millis(ValueB - ValueA));
  assert_eq!((time_b + delta_a), Timestamp::Millis(ValueB + ValueA));

  let mut mutable_time: Timestamp  =time_a;
  mutable_time += delta_b;
  assert_eq!(mutable_time, time_a + delta_b);
  mutable_time -= delta_b;
  assert_eq!(mutable_time, time_a);
}

#[test]
fn InfinityOperations() {
  const Value: i64 =  267;
  const finite_time: Timestamp = Timestamp::Millis(Value);
  const finite_delta: TimeDelta = TimeDelta::Millis(Value);
  assert!((Timestamp::PlusInfinity() + finite_delta).IsInfinite());
  assert!((Timestamp::PlusInfinity() - finite_delta).IsInfinite());
  assert!((finite_time + TimeDelta::PlusInfinity()).IsInfinite());
  assert!((finite_time - TimeDelta::MinusInfinity()).IsInfinite());
}
}  // namespace test
