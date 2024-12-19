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


 // TimeDelta represents the difference between two timestamps. Commonly this can
 // be a duration. However since two Timestamps are not guaranteed to have the
 // same epoch (they might come from different computers, making exact
 // synchronisation infeasible), the duration covered by a TimeDelta can be
 // undefined. To simplify usage, it can be constructed and converted to
 // different units, specifically seconds (s), milliseconds (ms) and
 // microseconds (us).
 super::relative_unit!(TimeDelta);

 impl TimeDelta {
    pub const fn Minutes<T: Into<Self>>(value: T) -> Self {
        Self::FromFraction(60_000_000, value)
    }

   pub const fn Seconds<T: Into<Self>>(value: T) -> Self {
     Self::FromFraction(1_000_000, value)
   }

   pub const fn Millis<T: Into<Self>>(value: T) -> Self {
     Self::FromFraction(1_000, value)
   }

   pub const fn Micros<T: Into<Self>>(value: T) -> Self {
     value.into()
   }

   pub const fn seconds<T: From<Self>>(&self) -> T {
     self.ToFraction(1_000_000)
   }

    pub const fn ms<T: From<Self>>(&self) -> T {
      self.ToFraction(1_000)
    }

    pub const fn us<T: From<Self>>(&self) -> T {
      T::from(self)
    }

    pub const fn ns<T: From<Self>>(&self) -> T {
      self.ToMultiple(1000)
    }

    pub const fn seconds_or(&self, fallback_value: i64) -> i64 {
      self.ToFractionOr(1_000_000, fallback_value)
    }

    pub const fn ms_or(&self, fallback_value: i64) -> i64 {
      self.ToFractionOr(1_000, fallback_value)
    }

    pub const fn us_or(&self, fallback_value: i64) -> i64 {
      self.try_into().unwrap_or(fallback_value)
    }

    pub const fn abs(&self) -> Self {
      return if self.us() < 0 {
        TimeDelta::Micros(-self.us())
      } else {
        *self
      };
    }
 }

 impl fmt::Debug for TimeDelta {
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
  const Value: i64 = -12345;
  const TimeDeltaZero: TimeDelta = TimeDelta::Zero();
  const TimeDeltaPlusInf: TimeDelta = TimeDelta::PlusInfinity();
  const TimeDeltaMinusInf: TimeDelta = TimeDelta::MinusInfinity();
  assert!(TimeDelta::default() == TimeDeltaZero);
  assert!(TimeDeltaZero.IsZero(), "");
  assert!(TimeDeltaPlusInf.IsPlusInfinity(), "");
  assert!(TimeDeltaMinusInf.IsMinusInfinity(), "");
  assert!(TimeDeltaPlusInf.ms_or(-1) == -1, "");

  assert!(TimeDeltaPlusInf > TimeDeltaZero, "");

  const TimeDeltaMinutes: TimeDelta = TimeDelta::Minutes(Value);
  const TimeDeltaSeconds: TimeDelta = TimeDelta::Seconds(Value);
  const TimeDeltaMs: TimeDelta = TimeDelta::Millis(Value);
  const TimeDeltaUs: TimeDelta = TimeDelta::Micros(Value);

  assert!(TimeDeltaMinutes.seconds_or(0) == Value * 60, "");
  assert!(TimeDeltaSeconds.seconds_or(0) == Value, "");
  assert!(TimeDeltaMs.ms_or(0) == Value, "");
  assert!(TimeDeltaUs.us_or(0) == Value, "");
}

#[test]
fn GetBacSameValues() {
  const Value: i64 = 499;
  for sign in [-1, 0, 1] {
    let value: i64 = Value * sign;
    assert_eq!(TimeDelta::Millis(value).ms(), value);
    assert_eq!(TimeDelta::Micros(value).us(), value);
    assert_eq!(TimeDelta::Seconds(value).seconds(), value);
    assert_eq!(TimeDelta::Seconds(value).seconds(), value);
  }
  assert_eq!(TimeDelta::Zero().us(), 0);
}

#[test]
fn GetDifferentPrefix() {
  const Value: i64 = 3000000;
  assert_eq!(TimeDelta::Micros(Value).seconds(), Value / 1000000);
  assert_eq!(TimeDelta::Millis(Value).seconds(), Value / 1000);
  assert_eq!(TimeDelta::Micros(Value).ms(), Value / 1000);
  assert_eq!(TimeDelta::Minutes(Value / 60).seconds(), Value);

  assert_eq!(TimeDelta::Millis(Value).us(), Value * 1000);
  assert_eq!(TimeDelta::Seconds(Value).ms(), Value * 1000);
  assert_eq!(TimeDelta::Seconds(Value).us(), Value * 1000000);
  assert_eq!(TimeDelta::Minutes(Value / 60).seconds(), Value);
}

#[test]
fn IdentityChecks() {
  const Value: i64 = 3000;
  assert!(TimeDelta::Zero().IsZero());
  assert!(!TimeDelta::Millis(Value).IsZero());

  assert!(TimeDelta::PlusInfinity().IsInfinite());
  assert!(TimeDelta::MinusInfinity().IsInfinite());
  assert!(!TimeDelta::Zero().IsInfinite());
  assert!(!TimeDelta::Millis(-Value).IsInfinite());
  assert!(!TimeDelta::Millis(Value).IsInfinite());

  assert!(!TimeDelta::PlusInfinity().IsFinite());
  assert!(!TimeDelta::MinusInfinity().IsFinite());
  assert!(TimeDelta::Millis(-Value).IsFinite());
  assert!(TimeDelta::Millis(Value).IsFinite());
  assert!(TimeDelta::Zero().IsFinite());

  assert!(TimeDelta::PlusInfinity().IsPlusInfinity());
  assert!(!TimeDelta::MinusInfinity().IsPlusInfinity());

  assert!(TimeDelta::MinusInfinity().IsMinusInfinity());
  assert!(!TimeDelta::PlusInfinity().IsMinusInfinity());
}

#[test]
fn ComparisonOperators() {
  const Small: i64 = 450;
  const Large: i64 = 451;
  const small: TimeDelta = TimeDelta::Millis(Small);
  const large: TimeDelta = TimeDelta::Millis(Large);

  assert_eq!(TimeDelta::Zero(), TimeDelta::Millis(0));
  assert_eq!(TimeDelta::PlusInfinity(), TimeDelta::PlusInfinity());
  assert_eq!(small, TimeDelta::Millis(Small));
  assert!(small <= TimeDelta::Millis(Small));
  assert!(small >= TimeDelta::Millis(Small));
  assert!(small != TimeDelta::Millis(Large));
  assert!(small <= TimeDelta::Millis(Large));
  assert!(small < TimeDelta::Millis(Large));
  assert!(large >= TimeDelta::Millis(Small));
  assert!(large > TimeDelta::Millis(Small));
  assert!(TimeDelta::Zero() < small);
  assert!(TimeDelta::Zero() > TimeDelta::Millis(-Small));
  assert!(TimeDelta::Zero() > TimeDelta::Millis(-Small));

  assert!(TimeDelta::PlusInfinity() > large);
  assert!(TimeDelta::MinusInfinity() < TimeDelta::Zero());
}

#[test]
fn Clamping() {
  const upper: TimeDelta = TimeDelta::Millis(800);
  const lower: TimeDelta = TimeDelta::Millis(100);
  const under: TimeDelta = TimeDelta::Millis(100);
  const inside: TimeDelta = TimeDelta::Millis(500);
  const over : TimeDelta= TimeDelta::Millis(1000);
  assert_eq!(under.Clamped(lower, upper), lower);
  assert_eq!(inside.Clamped(lower, upper), inside);
  assert_eq!(over.Clamped(lower, upper), upper);

  let mut mutable_delta: TimeDelta = lower;
  mutable_delta.Clamp(lower, upper);
  assert_eq!(mutable_delta, lower);
  mutable_delta = inside;
  mutable_delta.Clamp(lower, upper);
  assert_eq!(mutable_delta, inside);
  mutable_delta = over;
  mutable_delta.Clamp(lower, upper);
  assert_eq!(mutable_delta, upper);
}

#[test]
fn CanBeInititializedFromLargeInt() {
  const MaxInt: isize = isize::MAX;
  assert_eq!(TimeDelta::Seconds(MaxInt).us(),
            MaxInt as i64 * 1000000);
  assert_eq!(TimeDelta::Millis(MaxInt).us(),
            MaxInt as i64 * 1000);
}

#[test]
fn ConvertsToAndFromDouble() {
  const Micros: i64 = 17017;
  const NanosDouble: f64 = Micros as f64 * 1e3;
  const MicrosDouble: f64 = Micros as f64;
  const MillisDouble: f64 = Micros as f64 * 1e-3;
  const SecondsDouble: f64 = MillisDouble * 1e-3;

  assert_eq!(TimeDelta::Micros(Micros).seconds::<f64>(), SecondsDouble);
  assert_eq!(TimeDelta::Seconds(SecondsDouble).us(), Micros);

  assert_eq!(TimeDelta::Micros(Micros).ms::<f64>(), MillisDouble);
  assert_eq!(TimeDelta::Millis(MillisDouble).us(), Micros);

  assert_eq!(TimeDelta::Micros(Micros).us::<f64>(), MicrosDouble);
  assert_eq!(TimeDelta::Micros(MicrosDouble).us(), Micros);

  assert!((TimeDelta::Micros(Micros).ns::<f64>() - NanosDouble).abs() <= 1.0);

  const PlusInfinity: f64 = f64::INFINITY;
  const MinusInfinity: f64 = -PlusInfinity;

  assert_eq!(TimeDelta::PlusInfinity().seconds::<f64>(), PlusInfinity);
  assert_eq!(TimeDelta::MinusInfinity().seconds::<f64>(), MinusInfinity);
  assert_eq!(TimeDelta::PlusInfinity().ms::<f64>(), PlusInfinity);
  assert_eq!(TimeDelta::MinusInfinity().ms::<f64>(), MinusInfinity);
  assert_eq!(TimeDelta::PlusInfinity().us::<f64>(), PlusInfinity);
  assert_eq!(TimeDelta::MinusInfinity().us::<f64>(), MinusInfinity);
  assert_eq!(TimeDelta::PlusInfinity().ns::<f64>(), PlusInfinity);
  assert_eq!(TimeDelta::MinusInfinity().ns::<f64>(), MinusInfinity);

  assert!(TimeDelta::Seconds(PlusInfinity).IsPlusInfinity());
  assert!(TimeDelta::Seconds(MinusInfinity).IsMinusInfinity());
  assert!(TimeDelta::Millis(PlusInfinity).IsPlusInfinity());
  assert!(TimeDelta::Millis(MinusInfinity).IsMinusInfinity());
  assert!(TimeDelta::Micros(PlusInfinity).IsPlusInfinity());
  assert!(TimeDelta::Micros(MinusInfinity).IsMinusInfinity());
}

#[test]
fn MathOperations() {
  const ValueA: i64 = 267;
  const ValueB: i64 = 450;
  const delta_a: TimeDelta = TimeDelta::Millis(ValueA);
  const delta_b: TimeDelta = TimeDelta::Millis(ValueB);
  assert_eq!((delta_a + delta_b).ms(), ValueA + ValueB);
  assert_eq!((delta_a - delta_b).ms(), ValueA - ValueB);

  assert_eq!((delta_b / 10).ms(), ValueB / 10);
  assert_eq!(delta_b / delta_a, (ValueB) as f64 / ValueA);

  assert_eq!(TimeDelta::Micros(-ValueA).abs().us(), ValueA);
  assert_eq!(TimeDelta::Micros(ValueA).abs().us(), ValueA);

  let mut mutable_delta: TimeDelta = TimeDelta::Millis(ValueA);
  mutable_delta += TimeDelta::Millis(ValueB);
  assert_eq!(mutable_delta, TimeDelta::Millis(ValueA + ValueB));
  mutable_delta -= TimeDelta::Millis(ValueB);
  assert_eq!(mutable_delta, TimeDelta::Millis(ValueA));
}

#[test]
fn MultiplyByScalar() {
  const Value: TimeDelta = TimeDelta::Micros(267);
  const Int64: i64 = 450;
  const Int32: i32 = 123;
  const UnsignedInt: usize = 125;
  const Float: f64 = 123.0;

  assert_eq!((Value * Int64).us(), Value.us() * Int64);
  assert_eq!(Value * Int64, Int64 * Value);

  assert_eq!((Value * Int32).us(), Value.us() * Int32);
  assert_eq!(Value * Int32, Int32 * Value);

  assert_eq!(TimeDelta(Value * UnsignedInt).us(), Value.us() * UnsignedInt as i64);
  assert_eq!(Value * UnsignedInt, UnsignedInt * Value);

  assert!((((Value * Float).us()) - (Value.us() * Float)).abs() <= 0.1);
  assert_eq!(Value * Float, Float * Value);
}

#[test]
fn InfinityOperations() {
  const Value: i64 = 267;
  const finite: TimeDelta = TimeDelta::Millis(Value);
  assert!((TimeDelta::PlusInfinity() + finite).IsPlusInfinity());
  assert!((TimeDelta::PlusInfinity() - finite).IsPlusInfinity());
  assert!((finite + TimeDelta::PlusInfinity()).IsPlusInfinity());
  assert!((finite - TimeDelta::MinusInfinity()).IsPlusInfinity());

  assert!((TimeDelta::MinusInfinity() + finite).IsMinusInfinity());
  assert!((TimeDelta::MinusInfinity() - finite).IsMinusInfinity());
  assert!((finite + TimeDelta::MinusInfinity()).IsMinusInfinity());
  assert!((finite - TimeDelta::PlusInfinity()).IsMinusInfinity());
}
   }