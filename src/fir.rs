use std::{iter::Sum, ops::MulAssign};

use num::{traits::FloatConst, Float};

macro_rules! flt {
    ($x:expr) => {
        Flt::from($x).unwrap()
    };
}

// 计算sinc函数
fn sinc<T: Float + FloatConst>(x: T) -> T {
    if x == T::zero() {
        T::one()
    } else {
        (x * T::PI()).sin() / (x * T::PI())
    }
}

pub fn bessel_i0<Flt: Float>(x: Flt) -> Flt {
    let base = x * x / flt!(4);
    let mut addend = Flt::one();
    let mut sum = Flt::one();
    for j in 1.. {
        addend = addend * base / flt!(j * j);
        let old = sum;
        sum = sum + addend;
        if sum == old || !sum.is_finite() {
            break;
        }
    }
    sum
}
// 计算凯撒窗
fn kaiser_window<Flt: Float + FloatConst + MulAssign>(n: usize, beta: Flt) -> Vec<Flt> {
    let mid = Flt::from(n - 1).unwrap() / (Flt::one() + Flt::one());
    let denom = bessel_i0(beta);
    (0..n)
        .map(|i| {
            let i_t = flt!(i);
            let num = bessel_i0(
                beta * (Flt::one() - (flt!(4) * (i_t - mid).powi(2) / (flt!(n - 1)).powi(2)))
                    .sqrt(),
            );
            num / denom
        })
        .collect()
}

// 计算低通滤波器系数
pub fn design_lowpass_filter<Flt: Float + FloatConst + MulAssign + Sum>(
    ntap: usize,
    fcutoff: Flt,
    beta: Flt,
) -> Vec<Flt> {
    let mid = flt!(ntap - 1) / flt!(2);
    let kaiser_win = kaiser_window(ntap, beta);
    kaiser_win
        .iter()
        .enumerate()
        .map(|(n, &w)| {
            let h = sinc(flt!(2) * fcutoff * (flt!(n) - mid)); //# sinc(x) = sin(pi*x)/(pi*x)
            w * h
        })
        .collect()
}
