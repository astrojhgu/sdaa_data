use crossbeam::channel::{Receiver, Sender};
use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use num::Complex;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use std::f64::consts::PI;
use std::{collections::VecDeque, sync::Arc};

use crate::payload::N_PT_PER_FRAME;
use crate::{payload::Payload, Ftype};

const N: usize = N_PT_PER_FRAME; //=4096

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
pub struct DownConverter {
    coeff: Vec<Ftype>,
    inner_state: Vec<Complex<Ftype>>,
    dec_factor: usize,
    pub lo_table: [Complex<Ftype>; N],
    n: usize,
}

impl DownConverter {
    pub fn new(d: usize, k1: Ftype, beta: Ftype, fir_n: usize, lo_ch: usize) -> Self {
        let k = 0.5 / d as Ftype * k1;
        let coeff = design_lowpass_filter(fir_n, beta, k);

        let mut result = DownConverter {
            coeff,
            inner_state: Vec::with_capacity(N + fir_n * 2 + 1),
            dec_factor: d,
            lo_table: [Complex::default(); N],
            n: 0,
        };
        result.lo_table.iter_mut().enumerate().for_each(|(i, x)| {
            *x = Complex::<Ftype>::from_polar(
                1.0,
                (-(i as Ftype * lo_ch as Ftype) / N as Ftype) * 2.0 * PI as Ftype,
            );
        }); //填充本振数据

        result
    }

    pub fn process(
        &mut self,
        rx_raw: Receiver<LinearOwnedReusable<Payload>>,
        tx_ddc: Sender<LinearOwnedReusable<Vec<Complex<Ftype>>>>,
        pool: Arc<LinearObjectPool<Vec<Complex<Ftype>>>>,
    ) -> Vec<Complex<Ftype>> {
        loop {
            let payload = rx_raw.recv().unwrap(); //payload.data是一个[i16; N_PT_PER_FRAME]
            self.inner_state.extend(
                payload
                    .data
                    .iter()
                    .zip(self.lo_table.iter())
                    .map(|(&a, &b)| b * a as Ftype),
            ); //mixed signal

            let mut result = pool.pull_owned();
            result.drain(0..);

            // 滤波和下采样处理
            for i in (0..=self.inner_state.len() - self.coeff.len()).step_by(self.dec_factor) {
                let sum: Complex<Ftype> = self
                    .coeff
                    .iter()
                    .zip(&self.inner_state[i..])
                    .map(|(&c, &s)| c * s)
                    .sum();
                result.push(sum);
            }

            // 输出处理后的数据
            tx_ddc.send(result).unwrap();

            let l = self.inner_state.len();
            self.inner_state.copy_within(l - self.coeff.len() + 1.., 0);
            self.inner_state.drain(self.coeff.len() - 1..);
        }
    }
}
