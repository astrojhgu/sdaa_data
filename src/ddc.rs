use std::{simd::Simd, sync::Arc};

use crossbeam::channel::{Receiver, Sender};
use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use num::{
    traits::{FloatConst, Zero},
    Complex,
};

use crate::{
    fir::design_lowpass_filter,
    payload::{Payload, N_PT_PER_FRAME},
    Ftype,
};

pub fn ddc_x8(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<Ftype>>>>,
    lo_freq_ch: usize,
) {
    const NDEC: usize = 8;
    let pool = Arc::new(LinearObjectPool::<Vec<Complex<Ftype>>>::new(
        move || vec![Complex::default(); N_PT_PER_FRAME / NDEC],
        |_| {},
    ));

    const TAP: usize = 24;
    const TAP_PER_CH: usize = TAP / NDEC;

    let coeffs = design_lowpass_filter(TAP, 0.01, 5.0);
    let stat_len = NDEC * (TAP_PER_CH - 1);

    let lo: [Vec<_>; 2] = [
        (0..N_PT_PER_FRAME)
            .map(|i| {
                (lo_freq_ch as Ftype * i as Ftype / N_PT_PER_FRAME as Ftype * 2.0 * Ftype::PI())
                    .cos()
            })
            .collect(),
        (0..N_PT_PER_FRAME)
            .map(|i| {
                -(lo_freq_ch as Ftype * i as Ftype / N_PT_PER_FRAME as Ftype * 2.0 * Ftype::PI())
                    .sin()
            })
            .collect(),
    ];

    let mut buffer = [
        vec![Ftype::zero(); stat_len + N_PT_PER_FRAME],
        vec![Ftype::zero(); stat_len + N_PT_PER_FRAME],
    ];

    let mut result1 = [
        vec![Ftype::zero(); N_PT_PER_FRAME / NDEC],
        vec![Ftype::zero(); N_PT_PER_FRAME / NDEC],
    ];

    let c = Simd::<Ftype, TAP>::from_slice(coeffs.as_slice());

    loop {
        let payload = rx.recv().unwrap();
        let x = &payload.data;
        let mut result = pool.pull_owned();
        buffer
            .iter_mut()
            .zip(&mut result1)
            .zip(&lo)
            .enumerate()
            .for_each(|(j, ((b, r), lo1))| {
                b[stat_len..]
                    .iter_mut()
                    .zip(x.iter())
                    .zip(lo1.iter())
                    .for_each(|((b1, &x1), &lo1)| {
                        *b1 = (x1 as Ftype) * lo1;
                    });

                // let mut f =
                //     File::create(format!("buffer_{}.dat", if j == 0 { "i" } else { "q" })).unwrap();
                // f.write_all(slice_as_u8(&b)).unwrap();

                b.windows(TAP)
                    .step_by(NDEC)
                    .zip(r.iter_mut())
                    .for_each(|(x, r)| {
                        let x = Simd::<Ftype, TAP>::from_slice(x);
                        let z = x * c;
                        let z = z.to_array();
                        *r = z.iter().cloned().sum();
                        //*r = x.iter().zip(coeffs.iter()).map(|(&a, &b)| a * b).sum();
                        //*r = x[0];
                    });

                // let mut f =
                //     File::create(format!("filtered_{}.dat", if j == 0 { "i" } else { "q" }))
                //         .unwrap();
                // f.write_all(slice_as_u8(&r)).unwrap();

                //println!("{} {}", cnt1, cnt2);
                let stat_idx = b.len() - stat_len;
                b.copy_within(stat_idx.., 0);
            });

        result
            .iter_mut()
            .zip(result1[0].iter().zip(result1[1].iter()))
            .for_each(|(r, (&ri, &rq))| {
                r.re = ri;
                r.im = rq;
            });

        tx.send(result).unwrap();
    }
}
