use num::Complex;
use rand_distr::{Distribution, Normal};

use rand::rng;

use std::collections::VecDeque;
use std::f64::consts::PI;

const N: usize = 4096;
type Ftype = f32;

/// 计算凯撒窗
fn kaiser_window(n: usize, beta: Ftype) -> Vec<Ftype> {
    let mut window = Vec::with_capacity(2 * n + 1);
    let i0_beta = bessel_i0(beta); // 计算修正的零阶贝塞尔函数值

    for i in 0..(2 * n + 1) {
        let m = i as Ftype - n as Ftype;
        let ratio = m / n as Ftype;
        let value = bessel_i0(beta * (1.0 - ratio * ratio).sqrt()) / i0_beta;
        window.push(value);
    }
    window
}

/// 计算修正的零阶贝塞尔函数
fn bessel_i0(x: Ftype) -> Ftype {
    let mut sum = 1.0;
    let mut term = 1.0;
    let x_squared = x * x;
    let mut k = 1;

    // 使用级数展开计算
    loop {
        term *= x_squared / (4.0 * (k as Ftype).powi(2));
        sum += term;
        if term < 1e-20 * sum {
            break;
        }
        k += 1;
    }
    sum
}

/// 设计低通滤波器系数
fn design_lowpass_filter(n: usize, beta: Ftype, k: Ftype) -> Vec<Ftype> {
    let len = 2 * n + 1;
    let mut coefficients = vec![0.0; len];

    // 计算截止频率
    let cutoff_frequency = k; // 归一化截止频率

    // 计算理想低通滤波器的冲激响应
    for i in 0..len {
        let m = i as isize - n as isize;
        if m == 0 {
            coefficients[i] = 2.0 * cutoff_frequency;
        } else {
            coefficients[i] =
                (2.0 * cutoff_frequency * (PI as Ftype * cutoff_frequency * m as Ftype).sin())
                    / (PI as Ftype * cutoff_frequency * m as Ftype);
        }
    }

    // 应用凯撒窗
    let window = kaiser_window(n, beta);
    for i in 0..len {
        coefficients[i] *= window[i];
    }

    coefficients
}
struct DownConverter {
    coeff: Vec<Ftype>,
    inner_state: VecDeque<Complex<Ftype>>,
    dec_factor: usize,
    pub lo_iter: Box<dyn Iterator<Item = Complex<Ftype>>>,
    n: usize,
}

impl DownConverter {
    pub fn new(d: usize, k1: Ftype, beta: Ftype, fir_n: usize, lo_ch: usize) -> Self {
        let k = 0.5 / d as Ftype * k1;
        let coeff = design_lowpass_filter(fir_n, beta, k);
        let lo_iter = (0..N).map(move |i| {
            Complex::<Ftype>::new(
                0.0,
                (-(i as Ftype * lo_ch as Ftype) / N as Ftype) * 2.0 * PI as Ftype,
            )
            .exp()
        });
        DownConverter {
            coeff,
            inner_state: VecDeque::with_capacity(2 * fir_n + 1 + d),
            dec_factor: d,
            lo_iter: Box::new(lo_iter.cycle()),
            n: 0,
        }
    }

    pub fn process(&mut self, input: &[Ftype]) -> Vec<Complex<Ftype>> {
        self.inner_state
            .extend(input.iter().zip(&mut self.lo_iter).map(|(&a, b)| b * a));

        /*
        for x in &self.inner_state{
            println!("{} {}", x.re, x.im);
        } */

        //self.inner_state.extend(input);
        let mut result = Vec::with_capacity(input.len() / self.dec_factor + 1);

        while self.inner_state.len() >= self.coeff.len() {
            if self.n == 0 {
                let sum = self
                    .coeff
                    .iter()
                    .zip(self.inner_state.iter())
                    .map(|(&a, &b)| a * b)
                    .sum();
                result.push(sum);
            }

            // 一次性滑动窗口，避免频繁 pop_front()
            self.n += 1;
            if self.n == self.dec_factor {
                self.n = 0;
            }
            self.inner_state.drain(0..1); // 移动窗口
        }
        result
    }
}
fn main() {
    let f = 0.5 / N as f32 * 510.0;
    let signal:Vec<_>=(0..4096).map(|i| (2.0*PI as f32*f*i as f32).cos()).collect();
    
    let mut ddc = DownConverter::new(8, 0.5, 5.0, 20, 250);
    let result = ddc.process(&signal[..1000]);

    for x in result {
        println!("{} {}", x.re, x.im);
    }

    let result = ddc.process(&signal[1000..]);

    for x in result {
        println!("{} {}", x.re, x.im);
    }
}
