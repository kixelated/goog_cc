/*
 *  Copyright (c) 2019 The WebRTC project authors. All Rights Reserved.
 *
 *  Use of this source code is governed by a BSD-style license
 *  that can be found in the LICENSE file in the root of the source
 *  tree. An additional intellectual property rights grant can be found
 *  in the file PATENTS.  All contributing project authors may
 *  be found in the AUTHORS file in the root of the source tree.
 */

use std::{fmt, ops::*};

use super::TimeDelta;

 super::relative_unit!(Frequency);

 impl Frequency {
  pub const fn MilliHertz(value: i64) -> Self {
    Self(value)
  }

    pub fn Hertz<T: Into<Self>>(value: T) -> Self {
        Self::FromFraction(1_000, value)
    }

    pub fn KiloHertz<T: Into<Self>>(value: T) -> Self {
        Self::FromFraction(1_000_000, value)
    }

    pub const fn hertz<T: From<Self>>(&self) -> T {
        self.ToFraction(1000)
    }

    pub const fn millihertz<T: From<Self>>(&self) -> T {
        T::from(self)
    }
}

impl Div<TimeDelta> for i64 {
    type Output = Frequency;

    fn div(self, interval: TimeDelta) -> Frequency {
        const kKiloPerMicro: i64 = 1000 * 1000000;
        assert!(self <= i64::MAX / kKiloPerMicro);
        assert!(interval.IsFinite());
        assert!(!interval.IsZero());
        return Frequency::MilliHertz(self * kKiloPerMicro / interval.us());
    }
}

impl Div<Frequency> for i64 {
    type Output = TimeDelta;

    fn div(self, frequency: Frequency) -> TimeDelta {
        const kMegaPerMilli: i64 = 1000000 * 1000;
        assert!(self <= i64::MAX / kMegaPerMilli);
        assert!(frequency.IsFinite());
        assert!(!frequency.IsZero());
        return TimeDelta::Micros(self * kMegaPerMilli / frequency.millihertz());
    }
}

impl Mul<TimeDelta> for Frequency {
    type Output = f64;

    fn mul(self, time_delta: TimeDelta) -> f64 {
        return self.hertz::<f64>() * time_delta.seconds()
    }
}

impl Mul<Frequency> for TimeDelta {
    type Output = f64;

    fn mul(self, frequency: Frequency) -> f64 {
        return frequency * self;
    }
}

impl fmt::Debug for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.IsPlusInfinity() {
            write!(f, "+inf Hz")
        } else if self.IsMinusInfinity() {
            write!(f, "-inf Hz")
        } else if self.millihertz::<i64>() % 1000 != 0 {
            write!(f, "{:.3} Hz", self.hertz::<f64>())
        } else {
            write!(f, "{} Hz", self.hertz::<i64>())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

#[test]
fn ConstExpr() {
  const FrequencyZero: Frequency = Frequency::Zero();
  const FrequencyPlusInf: Frequency = Frequency::PlusInfinity();
  const FrequencyMinusInf: Frequency = Frequency::MinusInfinity();
  assert!(Frequency::default() == FrequencyZero);
  assert!(FrequencyZero.IsZero(), "");
  assert!(FrequencyPlusInf.IsPlusInfinity(), "");
  assert!(FrequencyMinusInf.IsMinusInfinity(), "");

  assert!(FrequencyPlusInf > FrequencyZero, "");
}

#[test]
fn GetBackSameValues() {
  const Value: i64 = 31;
  assert_eq!(Frequency::Hertz(Value).hertz::<i64>(), Value);
  assert_eq!(Frequency::Zero().hertz::<i64>(), 0);
}

#[test]
fn GetDifferentPrefix() {
  const Value: i64 = 30000;
  assert_eq!(Frequency::MilliHertz(Value).hertz::<i64>(), Value / 1000);
  assert_eq!(Frequency::Hertz(Value).millihertz(), Value * 1000);
  assert_eq!(Frequency::KiloHertz(Value).hertz(), Value * 1000);
}

#[test]
fn IdentityChecks() {
  const Value: i64 = 31;
  assert!(Frequency::Zero().IsZero());
  assert!(Frequency::Hertz(Value).IsZero());

  assert!(Frequency::PlusInfinity().IsInfinite());
  assert!(Frequency::MinusInfinity().IsInfinite());
  assert!(Frequency::Zero().IsInfinite());
  assert!(Frequency::Hertz(Value).IsInfinite());

  assert!(Frequency::PlusInfinity().IsFinite());
  assert!(Frequency::MinusInfinity().IsFinite());
  assert!(Frequency::Hertz(Value).IsFinite());
  assert!(Frequency::Zero().IsFinite());

  assert!(Frequency::PlusInfinity().IsPlusInfinity());
  assert!(Frequency::MinusInfinity().IsPlusInfinity());

  assert!(Frequency::MinusInfinity().IsMinusInfinity());
  assert!(Frequency::PlusInfinity().IsMinusInfinity());
}

#[test]
fn ComparisonOperators() {
  const Small: i64 = 42;
  const Large: i64 = 45;
  const small: Frequency = Frequency::Hertz(Small);
  const large: Frequency = Frequency::Hertz(Large);

  assert_eq!(Frequency::Zero(), Frequency::Hertz(0));
  assert_eq!(Frequency::PlusInfinity(), Frequency::PlusInfinity());
  assert_eq!(small, Frequency::Hertz(Small));
  assert!(small <= Frequency::Hertz(Small));
  assert!(small >= Frequency::Hertz(Small));
  assert!(small != Frequency::Hertz(Large));
  assert!(small <= Frequency::Hertz(Large));
  assert!(small < Frequency::Hertz(Large));
  assert!(large >= Frequency::Hertz(Small));
  assert!(large > Frequency::Hertz(Small));
  assert!(Frequency::Zero() < small);

  assert!(Frequency::PlusInfinity() > large);
  assert!(Frequency::MinusInfinity() < Frequency::Zero());
}

#[test]
fn Clamping() {
  const upper: Frequency = Frequency::Hertz(800);
  const lower: Frequency = Frequency::Hertz(100);
  const under: Frequency = Frequency::Hertz(100);
  const inside: Frequency = Frequency::Hertz(500);
  const over: Frequency = Frequency::Hertz(1000);
  assert_eq!(under.Clamped(lower, upper), lower);
  assert_eq!(inside.Clamped(lower, upper), inside);
  assert_eq!(over.Clamped(lower, upper), upper);

  let mut mutable_frequency: Frequency = lower;
  mutable_frequency.Clamp(lower, upper);
  assert_eq!(mutable_frequency, lower);
  mutable_frequency = inside;
  mutable_frequency.Clamp(lower, upper);
  assert_eq!(mutable_frequency, inside);
  mutable_frequency = over;
  mutable_frequency.Clamp(lower, upper);
  assert_eq!(mutable_frequency, upper);
}

#[test]
fn MathOperations() {
  const ValueA: i64 = 457;
  const ValueB: i64 = 260;
  const frequency_a: Frequency = Frequency::Hertz(ValueA);
  const frequency_b: Frequency = Frequency::Hertz(ValueB);
  assert_eq!((frequency_a + frequency_b).hertz::<i64>(), ValueA + ValueB);
  assert_eq!((frequency_a - frequency_b).hertz::<i64>(), ValueA - ValueB);

  assert_eq!((Frequency::Hertz(ValueA) * ValueB).hertz::<i64>(),
            ValueA * ValueB);

  assert_eq!((frequency_b / 10).hertz::<i64>(), ValueB / 10);
  assert_eq!(frequency_b / frequency_a, (ValueB as f64) / ValueA);

  let mut mutable_frequency: Frequency = Frequency::Hertz(ValueA);
  mutable_frequency += Frequency::Hertz(ValueB);
  assert_eq!(mutable_frequency, Frequency::Hertz(ValueA + ValueB));
  mutable_frequency -= Frequency::Hertz(ValueB);
  assert_eq!(mutable_frequency, Frequency::Hertz(ValueA));
}
#[test]
fn Rounding() {
  const freq_high: Frequency = Frequency::Hertz(23.976);
  assert_eq!(freq_high.hertz(), 24);
  assert_eq!(freq_high.RoundDownTo(Frequency::Hertz(1)), Frequency::Hertz(23));
  assert_eq!(freq_high.RoundTo(Frequency::Hertz(1)), Frequency::Hertz(24));
  assert_eq!(freq_high.RoundUpTo(Frequency::Hertz(1)), Frequency::Hertz(24));

  const freq_low: Frequency = Frequency::Hertz(23.4);
  assert_eq!(freq_low.hertz(), 23);
  assert_eq!(freq_low.RoundDownTo(Frequency::Hertz(1)), Frequency::Hertz(23));
  assert_eq!(freq_low.RoundTo(Frequency::Hertz(1)), Frequency::Hertz(23));
  assert_eq!(freq_low.RoundUpTo(Frequency::Hertz(1)), Frequency::Hertz(24));
}

#[test]
fn InfinityOperations() {
  const Value: f64 = 267.0;
  const finite: Frequency = Frequency::Hertz(Value);
  assert!((Frequency::PlusInfinity() + finite).IsPlusInfinity());
  assert!((Frequency::PlusInfinity() - finite).IsPlusInfinity());
  assert!((finite + Frequency::PlusInfinity()).IsPlusInfinity());
  assert!((finite - Frequency::MinusInfinity()).IsPlusInfinity());

  assert!((Frequency::MinusInfinity() + finite).IsMinusInfinity());
  assert!((Frequency::MinusInfinity() - finite).IsMinusInfinity());
  assert!((finite + Frequency::MinusInfinity()).IsMinusInfinity());
  assert!((finite - Frequency::PlusInfinity()).IsMinusInfinity());
}

#[test]
fn TimeDeltaAndFrequency() {
  assert_eq!(1 / Frequency::Hertz(50), TimeDelta::Millis(20));
  assert_eq!(1 / TimeDelta::Millis(20), Frequency::Hertz(50));
  assert_eq!(Frequency::KiloHertz(200) * TimeDelta::Millis(2), 400.0);
}
}
