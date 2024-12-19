// RelativeUnit is a superclass in C++.
// The closest we can do in Rust is a macro, as traits don't support const.
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
            type Output = Self;

            fn div(self, rhs: Self) -> Self::Output {
                Self::FromValueFloat(self.ToValueFloat() / rhs.ToValueFloat())
            }
        }

        impl ::std::ops::Div<f64> for $ty {
            type Output = Self;

            fn div(self, rhs: f64) -> Self::Output {
                Self::FromValueFloat(self.ToValueFloat() / rhs)
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
                Self::FromValueFloat(self.ToValueFloat() * rhs)
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
                Self::FromValueFloat(self.ToValueFloat() * rhs as f64)
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

    #[cfg(test)]
    mod test {

       use std::fmt;

    use super::*;

relative_unit!(TestUnit);

impl TestUnit {
    const ONE_SIDED: bool = true;

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

  pub const fn ToKiloFloatOr(&self, fallback: f64) -> f64 {
    self.ToFractionOrFloat(1000.0, fallback)
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
        write!(f, "{}", self.ToKilo())
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

  const kTestUnitKilo: TestUnit = TestUnit::FromValue(kValue);
  const kTestUnitValue: TestUnit = TestUnit::FromValue(kValue);

  assert!(kTestUnitKilo.ToKiloOr(0) == kValue);
  assert!(kTestUnitValue.ToValueOr(0) == kValue);
  assert!(TestUnitAddKilo(kTestUnitValue, 2).ToValue() == kValue + 2000);
  assert!(TestUnit::FromValue(500) / 2 == TestUnit::FromValue(250));
  assert!(TestUnit::FromValueFloat(500.0) / 2 == TestUnit::FromValueFloat(250.0));
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
  const delta_a: TestUnit = TestUnit::FromValue(kValueA);
  const delta_b: TestUnit = TestUnit::FromValue(kValueB);
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
  assert_eq!((delta_b / delta_a).ToValueFloat(), (kValueB / kValueA) as f64);

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
