use fftw::array::AlignedVec;
use fftw::plan::*;
use fftw::types::*;
use std::f64::consts::PI;

fn main() {
    let n = 32;
    let mut plan: C2CPlan64 = C2CPlan::aligned(&[n], Sign::Forward, Flag::MEASURE).unwrap();
    let mut a = AlignedVec::new(n * 4);
    let mut b = AlignedVec::new(n * 4);
    let k0 = 2.0 * PI / n as f64;
    for i in 0..n * 4 {
        a[i] = c64::new((k0 * i as f64).cos(), 0.0);
    }
    for i in 0..4{
        plan.c2c(&mut a[i*n..(i+1)*n], &mut b[i*n..(i+1)*n]).unwrap();
    }

    for i in 0..n*4{
        print!("{:E} ", b[i])
    }
}
