// RelativeUnit is a superclass in C++.
// The closest we can do in Rust is a macro, as traits don't support const.
macro_rules! relative_unit {
    ($ty:ident) => {
        crate::api::units::unit_base!($ty);

        impl $ty {
            pub const fn Clamped(&self, min_value: Self, max_value: Self) -> Self {
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
                Self(self.0 + rhs.0)
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
                Self(self.0 - rhs.0)
            }
        }

        impl ::std::ops::AddAssign for $ty {
            fn add_assign(&mut self, rhs: Self) {
                *self = *self + rhs;
            }
        }

        impl ::std::ops::SubAssign for $ty {
            fn sub_assign(&mut self, rhs: Self) {
                *self = *self - rhs;
            }
        }

        impl ::std::ops::Div for $ty {
            type Output = Self;

            fn div(self, rhs: Self) -> Self::Output {
                Self::from(f64::from(self) / f64::from(rhs))
            }
        }

        impl ::std::ops::Div<f64> for $ty {
            type Output = Self;

            fn div(self, rhs: f64) -> Self::Output {
                Self::from(f64::from(self) / rhs)
            }
        }

        impl ::std::ops::Div<i64> for $ty {
            type Output = Self;

            fn div(self, rhs: i64) -> Self::Output {
                Self::from(i64::from(self) / rhs)
            }
        }

        impl ::std::ops::Mul<f64> for $ty {
            type Output = Self;

            fn mul(self, rhs: f64) -> Self::Output {
                Self::from(f64::from(self) * rhs)
            }
        }

        impl ::std::ops::Mul<i64> for $ty {
            type Output = Self;

            fn mul(self, rhs: i64) -> Self::Output {
                Self::from(i64::from(self) * rhs)
            }
        }

        impl ::std::ops::Mul<i32> for $ty {
            type Output = Self;

            fn mul(self, rhs: i32) -> Self::Output {
                Self::from(i64::from(self) * i64::from(rhs))
            }
        }

    };
}

pub(crate) use relative_unit;

    #[cfg(test)]
    mod test {

       use super::*;

relative_unit!(TestUnit);

impl TestUnit {
  pub const fn FromKilo<T: Into<Self>>(kilo: T) -> Self {
    return Self::FromFraction(1000, kilo);
  }
  pub const fn ToKilo<T: From<Self>>(&self) -> T {
    return self.ToFraction(1000);
  }

  pub const fn ToKiloOr<T: From<Self>>(&self, fallback: T) -> T {
    return self.ToFractionOr(1000, fallback);
  }

  pub const fn ToMilli<T: From<Self>>(&self) -> T {
    return self.ToMultiple(1000);
  }
}

fn TestUnitAddKilo(mut value: TestUnit, add_kilo: i64) -> TestUnit {
  value += TestUnit::FromKilo(add_kilo);
  return value;
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
  assert!(TestUnit::from(0.0).IsZero());
  assert!(TestUnit::from(f64::INFINITY).IsPlusInfinity());
  assert!(TestUnit::from(-f64::INFINITY).IsMinusInfinity());
  assert!(TestUnit::from(250.0) == TestUnit::from(250));
  assert!(TestUnit::from(-250.0) == TestUnit::from(-250));

  assert!(kTestUnitPlusInf > kTestUnitZero);

  const kTestUnitKilo: TestUnit = TestUnit::FromKilo(kValue);
  const kTestUnitValue: TestUnit = TestUnit::from(kValue);

  assert!(kTestUnitKilo.ToKiloOr(0) == kValue);
  assert!(kTestUnitValue.try_into().unwrap_or(0) == kValue);
  assert!(TestUnitAddKilo(kTestUnitValue, 2).into() == kValue + 2000);
  assert!(TestUnit::from(500) / 2 == TestUnit::from(250));
  assert!(TestUnit::from(500.0) / 2 == TestUnit::from(250.0));
}

#[test]
fn Clamping() {
  const upper: TestUnit = TestUnit::FromKilo(800);
  const lower: TestUnit = TestUnit::FromKilo(100);
  const under: TestUnit = TestUnit::FromKilo(100);
  const inside: TestUnit = TestUnit::FromKilo(500);
  const over: TestUnit = TestUnit::FromKilo(1000);
  assert_eq!(under.Clamped(lower, upper), lower);
  assert_eq!(inside.Clamped(lower, upper), inside);
  assert_eq!(over.Clamped(lower, upper), upper);

  let mutable_delta: TestUnit = lower;
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
  assert_eq!((TestUnit::from(kValueA) * kValueB).into::<i64>(),
            kValueA * kValueB);
  assert_eq!((TestUnit::from(kValueA) * kInt32Value).into::<i64>(),
            kValueA * kInt32Value);
  assert_eq!((TestUnit::from(kValueA) * kFloatValue).into::<i64>(),
            kValueA * kFloatValue);

  assert_eq!((delta_b / 10).ToKilo(), kValueB / 10);
  assert_eq!(delta_b / delta_a, (kValueB as f64) / kValueA);

  let mutable_delta: TestUnit = TestUnit::FromKilo(kValueA);
  mutable_delta += TestUnit::FromKilo(kValueB);
  assert_eq!(mutable_delta, TestUnit::FromKilo(kValueA + kValueB));
  mutable_delta -= TestUnit::FromKilo(kValueB);
  assert_eq!(mutable_delta, TestUnit::FromKilo(kValueA));

  // Division by an int rounds towards zero to follow regular int division.
  assert_eq!(TestUnit::from(789) / 10, TestUnit::from(78));
  assert_eq!(TestUnit::from(-789) / 10, TestUnit::from(-78));
}

#[test]
fn InfinityOperations() {
  const kValue: i64 = 267;
  const finite: TestUnit = TestUnit::FromKilo(kValue);
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
  const unit: TestUnit = TestUnit::from(kValue);
  assert_eq!(-unit.into(), -kValue);

  // Check infinity.
  assert_eq!(-TestUnit::PlusInfinity(), TestUnit::MinusInfinity());
  assert_eq!(-TestUnit::MinusInfinity(), TestUnit::PlusInfinity());
}

    }
