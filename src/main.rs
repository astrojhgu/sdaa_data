use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;

fn main() {
    let length = 1024;
    let mut real_planner = RealFftPlanner::<f32>::new();

    let r2c = real_planner.plan_fft_forward(length);
    let mut indata = r2c.make_input_vec();
    let mut rawdata = vec![0_i16; indata.len()];

    let mut spectrum = r2c.make_output_vec();

    let n = (480e6 / length as f64) as usize + 1;

    for i in 0..n * 10 {
        rawdata[2] = i as i16;
        rawdata.iter().zip(indata.iter_mut()).for_each(|(a,b)|{
            *b=*a as f32;
        });
        r2c.process(&mut indata, &mut spectrum).unwrap();
        //println!("{}", spectrum[2]);
    }
}
