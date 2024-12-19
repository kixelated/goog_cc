// UnitBase is a superclass in C++.
// The closest we can do in Rust is a macro, as traits don't support const.
 macro_rules! unit_base {
     ($ty:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
        pub struct $ty(i64);

        impl $ty {
            pub const fn Zero() -> Self {
                Self(0)
            }
            pub const fn PlusInfinity() -> Self {
                Self(i64::MAX)
            }
            pub const fn MinusInfinity() -> Self {
                Self(i64::MIN)
            }

            pub const fn IsZero(&self) -> bool {
                self.0 == 0
            }
            pub const fn IsFinite(&self) -> bool {
                !self.IsInfinite()
            }
            pub const fn IsInfinite(&self) -> bool {
                self.0 == i64::MAX || self.0 == i64::MIN
            }

            pub const fn IsPlusInfinity(&self) -> bool {
                self.0 == i64::MAX
            }

            pub const fn IsMinusInfinity(&self) -> bool {
                self.0 == i64::MIN
            }

            pub const fn RoundTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self::FromValue(((self.0 + resolution.0 / 2) / resolution.0) * resolution.0)
            }

            pub const fn RoundUpTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self::FromValue(((self.0 + resolution.0 - 1) / resolution.0) * resolution.0)
            }

            pub const fn RoundDownTo(&self, resolution: Self) -> Self{
                assert!(self.IsFinite());
                assert!(resolution.IsFinite());
                assert!(resolution.0 > 0);
                Self::FromValue((self.0 / resolution.0) * resolution.0)
            }

            const fn FromFraction(denominator: i64, value: i64) -> Self {
                assert!(denominator >= 0);
                Self::FromValue(value * denominator)
            }

            const fn FromFractionFloat(denominator: f64, value: f64) -> Self {
                Self::FromValueFloat(value * denominator)
            }

            const fn ToFraction(&self, denominator: i64) -> i64 {
                self.DivideRoundToNearest(denominator)
            }

            const fn DivideRoundToNearest(&self, d: i64) -> i64 {
                assert!(d >= 0);

                let v = self.ToValue();
                let mut result = v / d;
                let remainder = v % d;

                if remainder.abs() * 2 >= d.abs() {
                    if (v < 0) != (d < 0) {
                        result -= 1
                    } else {
                        result += 1
                    }
                }
                result
            }

            const fn ToFractionFloat(&self, denominator: f64) -> f64 {
                assert!(denominator >= 0.0);
                self.ToValueFloat() / denominator
            }

            const fn ToFractionOr(&self, denominator: i64, fallback_value: i64) -> i64 {
                assert!(denominator >= 0);
                if self.IsFinite() {
                    self.DivideRoundToNearest(denominator)
                } else {
                    fallback_value
                }
            }

            pub const fn ToMultiple(&self, factor: i64) -> i64 {
                assert!(factor >= 0);
                self.ToValue() * factor
            }

            pub const fn ToMultipleFloat(&self, factor: f64) -> f64 {
                assert!(factor >= 0.0);
                self.ToValueFloat() * factor
            }

            const fn FromValue(value: i64) -> Self {
                assert!(value != i64::MAX && value != i64::MIN);
                if Self::ONE_SIDED {
                    assert!(value >= 0);
                }

                Self(value)
            }

            const fn FromValueFloat(value: f64) -> Self {
                assert!(!value.is_nan());

                if value == f64::INFINITY {
                    return Self::PlusInfinity();
                }

                if Self::ONE_SIDED {
                    assert!(value >= 0.0);
                }

                if value == f64::NEG_INFINITY {
                    Self::MinusInfinity()
                } else {
                    Self(value as i64)
                }
            }

            const fn ToValue(&self) -> i64 {
                assert!(self.IsFinite());
                self.0
            }

            const fn ToValueOr(&self, fallback_value: i64) -> i64 {
                if self.IsFinite() {
                    self.0
                } else {
                    fallback_value
                }
            }

            const fn ToValueFloat(&self) -> f64 {
                if self.IsPlusInfinity() {
                    f64::INFINITY
                } else if self.IsMinusInfinity() {
                    f64::NEG_INFINITY
                } else {
                    self.0 as f64
                }
            }
        }
     };
 }

macro_rules! relative_unit {
    ($ty:ident) => {
        crate::api::units::unit_base!($ty);

        impl $ty {
            pub fn Clamped(&self, min_value: Self, max_value: Self) -> Self {
                Self(self.0.max(min_value.0).min(max_value.0))
            }

            pub fn Clamp(&mut self, min_value: Self, max_value: Self) {
                *self = Self(self.0.max(min_value.0).min(max_value.0));
            }
        }

        impl ::std::ops::Add for $ty {
            type Output = Self;

            fn add(self, rhs: Self) -> Self::Output {
                if self.IsPlusInfinity() || rhs.IsPlusInfinity() {
                    assert!(!self.IsMinusInfinity());
                    assert!(!rhs.IsMinusInfinity());
                    return Self::PlusInfinity();
                } else if self.IsMinusInfinity() || rhs.IsMinusInfinity() {
                    assert!(!self.IsPlusInfinity());
                    assert!(!rhs.IsPlusInfinity());
                    return Self::MinusInfinity();
                }
                Self::FromValue(self.ToValue() + rhs.ToValue())
            }
        }

        impl ::std::ops::Sub for $ty {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self::Output {
                if self.IsPlusInfinity() || rhs.IsMinusInfinity() {
                    assert!(!self.IsMinusInfinity());
                    assert!(!rhs.IsPlusInfinity());
                    return Self::PlusInfinity();
                } else if self.IsMinusInfinity() || rhs.IsPlusInfinity() {
                    assert!(!self.IsPlusInfinity());
                    assert!(!rhs.IsMinusInfinity());
                    return Self::MinusInfinity();
                }
                Self::FromValue(self.ToValue() - rhs.ToValue())
            }
        }

        impl ::std::ops::AddAssign for $ty {
            fn add_assign(&mut self, rhs: Self) {
                *self = Self::FromValue(self.ToValue() + rhs.ToValue());
            }
        }

        impl ::std::ops::SubAssign for $ty {
            fn sub_assign(&mut self, rhs: Self) {
                *self = Self::FromValue(self.ToValue() - rhs.ToValue());
            }
        }

        impl ::std::ops::Div for $ty {
            type Output = f64;

            fn div(self, rhs: Self) -> Self::Output {
                self.ToValueFloat() / rhs.ToValueFloat()
            }
        }

        impl ::std::ops::Div<f64> for $ty {
            type Output = Self;

            fn div(self, rhs: f64) -> Self::Output {
                Self::FromValueFloat((self.ToValueFloat() / rhs).round())
            }
        }

        impl ::std::ops::Div<i64> for $ty {
            type Output = Self;

            fn div(self, rhs: i64) -> Self::Output {
                Self::FromValue(self.ToValue() / rhs)
            }
        }

        impl ::std::ops::Mul<f64> for $ty {
            type Output = Self;

            fn mul(self, rhs: f64) -> Self::Output {
                Self::FromValueFloat((self.ToValueFloat() * rhs).round())
            }
        }

        impl ::std::ops::Mul<i64> for $ty {
            type Output = Self;

            fn mul(self, rhs: i64) -> Self::Output {
                Self::FromValue(self.ToValue() * rhs)
            }
        }

        impl ::std::ops::Mul<i32> for $ty {
            type Output = Self;

            fn mul(self, rhs: i32) -> Self::Output {
                Self::FromValue(self.ToValue() * rhs as i64)
            }
        }

        impl ::std::ops::Mul<f32> for $ty {
            type Output = Self;

            fn mul(self, rhs: f32) -> Self::Output {
                Self::FromValueFloat((self.ToValueFloat() * rhs as f64).round())
            }
        }

        impl ::std::ops::Mul<usize> for $ty {
            type Output = Self;

            fn mul(self, rhs: usize) -> Self::Output {
                Self::FromValue(self.ToValue() * i64::try_from(rhs).unwrap())
            }
        }

        impl ::std::ops::Neg for $ty {
            type Output = Self;

            fn neg(self) -> Self::Output {
                if self.IsPlusInfinity() {
                    Self::MinusInfinity()
                } else if self.IsMinusInfinity() {
                    Self::PlusInfinity()
                } else {
                    Self::FromValue(-self.ToValue())
                }
            }
        }

        impl ::std::ops::Mul<$ty> for i64 {
            type Output = $ty;

            fn mul(self, rhs: $ty) -> Self::Output {
                rhs * self
            }
        }

        impl ::std::ops::Mul<$ty> for i32 {
            type Output = $ty;

            fn mul(self, rhs: $ty) -> Self::Output {
                rhs * self
            }
        }

        impl ::std::ops::Mul<$ty> for usize {
            type Output = $ty;

            fn mul(self, rhs: $ty) -> Self::Output {
                rhs * self
            }
        }

        impl ::std::ops::Mul<$ty> for f64 {
            type Output = $ty;

            fn mul(self, rhs: $ty) -> Self::Output {
                rhs * self
            }
        }

        impl ::std::ops::Mul<$ty> for f32 {
            type Output = $ty;

            fn mul(self, rhs: $ty) -> Self::Output {
                rhs * self
            }
        }

    };
}

pub(crate) use relative_unit;
pub(crate) use unit_base;

    #[cfg(test)]
    mod test {

       use std::fmt;

    use super::*;

relative_unit!(TestUnit);

impl TestUnit {
    const ONE_SIDED: bool = false;

    pub const fn FromKilo(kilo: i64) -> Self {
        Self::FromFraction(1000, kilo)
    }

    pub fn FromKiloFloat(kilo: f64) -> Self {
        Self::FromFractionFloat(1000.0, kilo)
    }
  pub const fn ToKilo(&self) -> i64 {
    self.ToFraction(1000)
  }

  pub const fn ToKiloFloat(&self) -> f64 {
    self.ToFractionFloat(1000.0)
  }

  pub const fn ToKiloOr(&self, fallback: i64) -> i64 {
    self.ToFractionOr(1000, fallback)
  }

  pub const fn ToMilli(&self) -> i64 {
    self.ToMultiple(1000)
  }

    pub const fn ToMilliFloat(&self) -> f64 {
        self.ToMultipleFloat(1000.0)
    }
}

impl fmt::Debug for TestUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.IsPlusInfinity() {
            write!(f, "+inf")
        } else if self.IsMinusInfinity() {
            write!(f, "-inf")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

fn TestUnitAddKilo(mut value: TestUnit, add_kilo: i64) -> TestUnit {
  value += TestUnit::FromKilo(add_kilo);
  value
}

#[test]
fn ConstExpr() {
  const kValue: i64 = -12345;
  const kTestUnitZero: TestUnit = TestUnit::Zero();
  const kTestUnitPlusInf: TestUnit = TestUnit::PlusInfinity();
  const kTestUnitMinusInf: TestUnit = TestUnit::MinusInfinity();

  assert!(kTestUnitZero.IsZero());
  assert!(kTestUnitPlusInf.IsPlusInfinity());
  assert!(kTestUnitMinusInf.IsMinusInfinity());
  assert!(kTestUnitPlusInf.ToKiloOr(-1) == -1);

  // Check from is constexpr for floats.
  assert!(TestUnit::FromValueFloat(0.0).IsZero());
  assert!(TestUnit::FromValueFloat(f64::INFINITY).IsPlusInfinity());
  assert!(TestUnit::FromValueFloat(-f64::INFINITY).IsMinusInfinity());
  assert!(TestUnit::FromValueFloat(250.0) == TestUnit::FromValue(250));
  assert!(TestUnit::FromValueFloat(-250.0) == TestUnit::FromValue(-250));

  assert!(kTestUnitPlusInf > kTestUnitZero);

  const kTestUnitKilo: TestUnit = TestUnit::FromKilo(kValue);
  const kTestUnitValue: TestUnit = TestUnit::FromValue(kValue);

  assert!(kTestUnitKilo.ToKiloOr(0) == kValue);
  assert!(kTestUnitValue.ToValueOr(0) == kValue);
  assert!(TestUnitAddKilo(kTestUnitValue, 2).ToValue() == kValue + 2000);
  assert!(TestUnit::FromValue(500) / 2 == TestUnit::FromValue(250));
  assert!(TestUnit::FromValueFloat(500.0) / 2 == TestUnit::FromValueFloat(250.0));
}

#[test]
fn GetBackSameValues() {
  const kValue: i64 = 499;
  for sign in [-1, 0, 1] {
    let value: i64 = kValue * sign;
    assert_eq!(TestUnit::FromKilo(value).ToKilo(), value);
    assert_eq!(TestUnit::FromValue(value).ToValue(), value);
  }
  assert_eq!(TestUnit::Zero().ToValue(), 0);
}

#[test]
fn GetDifferentPrefix() {
  const kValue: i64 = 3000000;
  assert_eq!(TestUnit::FromValue(kValue).ToKilo(), kValue / 1000);
  assert_eq!(TestUnit::FromKilo(kValue).ToValue(), kValue * 1000);
}

#[test]
fn IdentityChecks() {
  const kValue: i64 = 3000;
  assert!(TestUnit::Zero().IsZero());
  assert!(!TestUnit::FromKilo(kValue).IsZero());

  assert!(TestUnit::PlusInfinity().IsInfinite());
  assert!(TestUnit::MinusInfinity().IsInfinite());
  assert!(!TestUnit::Zero().IsInfinite());
  assert!(!TestUnit::FromKilo(-kValue).IsInfinite());
  assert!(!TestUnit::FromKilo(kValue).IsInfinite());

  assert!(!TestUnit::PlusInfinity().IsFinite());
  assert!(!TestUnit::MinusInfinity().IsFinite());
  assert!(TestUnit::FromKilo(-kValue).IsFinite());
  assert!(TestUnit::FromKilo(kValue).IsFinite());
  assert!(TestUnit::Zero().IsFinite());

  assert!(TestUnit::PlusInfinity().IsPlusInfinity());
  assert!(!TestUnit::MinusInfinity().IsPlusInfinity());

  assert!(TestUnit::MinusInfinity().IsMinusInfinity());
  assert!(!TestUnit::PlusInfinity().IsMinusInfinity());
}

#[test]
fn ComparisonOperators() {
  const kSmall: i64 = 450;
  const kLarge: i64 = 451;
  const small: TestUnit = TestUnit::FromKilo(kSmall);
  const large: TestUnit = TestUnit::FromKilo(kLarge);

  assert_eq!(TestUnit::Zero(), TestUnit::FromKilo(0));
  assert_eq!(TestUnit::PlusInfinity(), TestUnit::PlusInfinity());
  assert_eq!(small, TestUnit::FromKilo(kSmall));
  assert!(small <= TestUnit::FromKilo(kSmall));
  assert!(small >= TestUnit::FromKilo(kSmall));
  assert!(small != TestUnit::FromKilo(kLarge));
  assert!(small <= TestUnit::FromKilo(kLarge));
  assert!(small < TestUnit::FromKilo(kLarge));
  assert!(large >= TestUnit::FromKilo(kSmall));
  assert!(large > TestUnit::FromKilo(kSmall));
  assert!(TestUnit::Zero() < small);
  assert!(TestUnit::Zero() > TestUnit::FromKilo(-kSmall));
  assert!(TestUnit::Zero() > TestUnit::FromKilo(-kSmall));

  assert!(TestUnit::PlusInfinity() > large);
  assert!(TestUnit::MinusInfinity() < TestUnit::Zero());
}

#[test]
fn CanBeInititializedFromLargeInt() {
  const kMaxInt: i32 = i32::MAX;
  assert_eq!(TestUnit::FromKilo(kMaxInt as i64).ToValue(),
            (kMaxInt as i64) * 1000);
}

#[test]
fn ConvertsToAndFromDouble() {
  const kValue: i64 = 17017;
  const kMilliDouble: f64 = kValue as f64 * 1e3;
  const kValueDouble: f64 = kValue as f64;
  const kKiloDouble: f64 = kValue as f64 * 1e-3;

  assert_eq!(TestUnit::FromValue(kValue).ToKiloFloat(), kKiloDouble);
  assert_eq!(TestUnit::FromKiloFloat(kKiloDouble).ToValue(), kValue);

  assert_eq!(TestUnit::FromValue(kValue).ToValueFloat(), kValueDouble);
  assert_eq!(TestUnit::FromValueFloat(kValueDouble).ToValue(), kValue);

  assert!((TestUnit::FromValue(kValue).ToMilliFloat() - kMilliDouble).abs() <= 1.0);

  const kPlusInfinity: f64 = f64::INFINITY;
  const kMinusInfinity: f64 = -kPlusInfinity;

  assert_eq!(TestUnit::PlusInfinity().ToKiloFloat(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToKiloFloat(), kMinusInfinity);
  assert_eq!(TestUnit::PlusInfinity().ToKiloFloat(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToKiloFloat(), kMinusInfinity);
  assert_eq!(TestUnit::PlusInfinity().ToMilliFloat(), kPlusInfinity);
  assert_eq!(TestUnit::MinusInfinity().ToMilliFloat(), kMinusInfinity);

  assert!(TestUnit::FromKiloFloat(kPlusInfinity).IsPlusInfinity());
  assert!(TestUnit::FromKiloFloat(kMinusInfinity).IsMinusInfinity());
  assert!(TestUnit::FromValueFloat(kPlusInfinity).IsPlusInfinity());
  assert!(TestUnit::FromValueFloat(kMinusInfinity).IsMinusInfinity());
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan1() {
  TestUnit::FromValueFloat(f64::NAN);
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan2() {
  TestUnit::FromValueFloat(0.0 / 0.0);
}

#[test]
#[should_panic]
fn CrashesWhenCreatedFromNan3() {
  TestUnit::FromValueFloat(f64::INFINITY - f64::INFINITY);
}




#[test]
fn Clamping() {
  const upper: TestUnit = TestUnit::FromValue(800);
  const lower: TestUnit = TestUnit::FromValue(100);
  const under: TestUnit = TestUnit::FromValue(100);
  const inside: TestUnit = TestUnit::FromValue(500);
  const over: TestUnit = TestUnit::FromValue(1000);
  assert_eq!(under.Clamped(lower, upper), lower);
  assert_eq!(inside.Clamped(lower, upper), inside);
  assert_eq!(over.Clamped(lower, upper), upper);

  let mut mutable_delta: TestUnit = lower;
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
fn MathOperations() {
  const kValueA: i64 = 267;
  const kValueB: i64 = 450;
  const delta_a: TestUnit = TestUnit::FromKilo(kValueA);
  const delta_b: TestUnit = TestUnit::FromKilo(kValueB);
  assert_eq!((delta_a + delta_b).ToKilo(), kValueA + kValueB);
  assert_eq!((delta_a - delta_b).ToKilo(), kValueA - kValueB);

  const kInt32Value: i32 = 123;
  const kFloatValue: f64 = 123.0;
  assert_eq!((TestUnit::FromValue(kValueA) * kValueB).ToValue(),
            kValueA * kValueB);
  assert_eq!((TestUnit::FromValue(kValueA) * kInt32Value).ToValue(),
            kValueA * kInt32Value as i64);
  assert_eq!((TestUnit::FromValue(kValueA) * kFloatValue).ToValue(),
            kValueA * kFloatValue as i64);

  assert_eq!((delta_b / 10).ToKilo(), kValueB / 10);
  assert_eq!(delta_b / delta_a, kValueB as f64 / kValueA as f64);

  let mut mutable_delta: TestUnit = TestUnit::FromKilo(kValueA);
  mutable_delta += TestUnit::FromKilo(kValueB);
  assert_eq!(mutable_delta, TestUnit::FromKilo(kValueA + kValueB));
  mutable_delta -= TestUnit::FromKilo(kValueB);
  assert_eq!(mutable_delta, TestUnit::FromKilo(kValueA));

  // Division by an int rounds towards zero to follow regular int division.
  assert_eq!(TestUnit::FromValue(789) / 10, TestUnit::FromValue(78));
  assert_eq!(TestUnit::FromValue(-789) / 10, TestUnit::FromValue(-78));
}

#[test]
fn InfinityOperations() {
  const kValue: i64 = 267;
  const finite: TestUnit = TestUnit::FromValue(kValue);
  assert!((TestUnit::PlusInfinity() + finite).IsPlusInfinity());
  assert!((TestUnit::PlusInfinity() - finite).IsPlusInfinity());
  assert!((finite + TestUnit::PlusInfinity()).IsPlusInfinity());
  assert!((finite - TestUnit::MinusInfinity()).IsPlusInfinity());

  assert!((TestUnit::MinusInfinity() + finite).IsMinusInfinity());
  assert!((TestUnit::MinusInfinity() - finite).IsMinusInfinity());
  assert!((finite + TestUnit::MinusInfinity()).IsMinusInfinity());
  assert!((finite - TestUnit::PlusInfinity()).IsMinusInfinity());
}

#[test]
fn UnaryMinus() {
  const kValue: i64 = 1337;
  const unit: TestUnit = TestUnit::FromValue(kValue);
  assert_eq!(-unit.ToValue(), -kValue);

  // Check infinity.
  assert_eq!(-TestUnit::PlusInfinity(), TestUnit::MinusInfinity());
  assert_eq!(-TestUnit::MinusInfinity(), TestUnit::PlusInfinity());
}

    }
