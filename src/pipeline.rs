use std::time::{Duration, Instant};
use std::{net::UdpSocket, sync::Arc};

use chrono::Local;
use crossbeam::channel::{Receiver, Sender};
use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use rustfft::num_complex::Complex;
use rustfft::{FftPlanner, FftPlannerAvx};

#[cfg(not(feature = "no_cuda"))]
use crate::ddc::{fcomplex, DownConverter, DownConverter2};

use crate::{
    payload::{Payload, N_PT_PER_FRAME},
    utils::as_mut_u8_slice,
};

pub fn recv_pkt(socket: UdpSocket, tx: Sender<LinearOwnedReusable<Payload>>) {
    let mut last_print_time = Instant::now();
    let print_interval = Duration::from_secs(2);

    let mut next_cnt = None;
    let mut ndropped = 0;
    let pool: Arc<LinearObjectPool<Payload>> = Arc::new(LinearObjectPool::new(
        move || {
            //eprint!(".");
            Payload::default()
        },
        |v| {
            v.pkt_cnt = 0;
            v.data.fill(0);
        },
    ));

    loop {
        let now = Instant::now();

        if now.duration_since(last_print_time) >= print_interval {
            let local_time = Local::now().format("%Y-%m-%d %H:%M:%S");
            println!("{} {} pkts dropped q={}", local_time, ndropped, tx.len());
            last_print_time = now;
        }
        let mut payload = pool.pull_owned();
        let buf = as_mut_u8_slice(&mut payload as &mut Payload);
        let (s, _a) = socket.recv_from(buf).unwrap();
        if s != std::mem::size_of::<Payload>() {
            continue;
        }
        if next_cnt.is_none() {
            next_cnt = Some(payload.pkt_cnt);
            ndropped = 0;
        }

        if payload.pkt_cnt == 0 {
            ndropped = 0;
            let local_time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            println!();
            println!("==================================");
            println!("start time:{}", local_time);
            println!("==================================");
        }

        while let Some(ref mut c) = next_cnt {
            //let current_cnt = c + 1;
            if *c >= payload.pkt_cnt {
                *c = payload.pkt_cnt + 1;
                if tx.is_full() {
                    //eprint!("o");
                    continue;
                }
                if let Ok(()) = tx.send(payload) {
                    break;
                } else {
                    return;
                }
            }

            ndropped += 1;

            let mut payload1 = pool.pull_owned();
            payload1.copy_header(&payload);
            payload1.pkt_cnt = *c;

            if tx.is_full() {
                //eprint!("o");
                continue;
            }

            if tx.send(payload1).is_err() {
                return;
            }
            *c += 1;
        }
    }
}

pub fn pkt_fft(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<f32>>>>,
    nch: usize,
) {
    assert!(N_PT_PER_FRAME % (nch * 2) == 0 || (nch * 2) % N_PT_PER_FRAME == 0);
    let nbuf = (nch * 2).max(N_PT_PER_FRAME);
    let pool: Arc<LinearObjectPool<Vec<Complex<f32>>>> = Arc::new(LinearObjectPool::new(
        move || {
            eprint!(".");
            vec![Complex::default(); nbuf / 2]
        },
        |v| {},
    ));

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(nch * 2);

    let mut buffer = vec![Complex::<f32>::default(); nbuf];
    let mut offset = 0;
    while let Ok(payload) = rx.recv() {
        buffer[offset..(offset + N_PT_PER_FRAME)]
            .iter_mut()
            .zip(payload.data.iter())
            .for_each(|(a, &b)| {
                *a = (b as f32).into();
                //*a=(b as f32).into();
            });
        offset += N_PT_PER_FRAME;
        if offset == nbuf {
            offset = 0;
            fft.process(&mut buffer);
            let mut result = pool.pull_owned();
            (&buffer)
                .chunks(nch)
                .step_by(2)
                .zip((&mut result).chunks_mut(nch))
                .for_each(|(a, b)| {
                    b.copy_from_slice(a);
                });

            //tx.try_send(result).unwrap();
            if tx.send(result).is_err() {
                break;
            }
        }
    }
}

pub fn pkt_integrate(
    rx: Receiver<LinearOwnedReusable<Vec<Complex<f32>>>>,
    tx: Sender<LinearOwnedReusable<Vec<f32>>>,
    nint: usize,
) {
    let pool: Arc<LinearObjectPool<Vec<f32>>> = Arc::new(LinearObjectPool::new(
        move || {
            eprint!(".");
            vec![]
        },
        |v| {},
    ));

    loop {
        let mut result = pool.pull_owned();
        for i in 0..nint {
            if let Ok(x) = rx.recv() {
                if result.is_empty() {
                    result.resize(x.len(), 0.0);
                } else if i == 0 {
                    result
                        .iter_mut()
                        .zip(x.iter())
                        .for_each(|(a, &b)| *a = b.norm_sqr());
                } else {
                    result
                        .iter_mut()
                        .zip(x.iter())
                        .for_each(|(a, &b)| *a += b.norm_sqr());
                }
            } else {
                return;
            }
        }
        if tx.send(result).is_err() {
            return;
        }
    }
}

#[cfg(not(feature = "no_cuda"))]
pub fn pkt_ddc(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<f32>>>>,
    ndec: usize,
    rx_lo_ch: Receiver<isize>,
    fir_coeffs: &[f32],
) {
    let mut ddc = DownConverter::new(ndec, fir_coeffs);
    let n_out_data = ddc.n_out_data();
    let pool: Arc<LinearObjectPool<Vec<Complex<f32>>>> = Arc::new(LinearObjectPool::new(
        move || {
            eprint!(".");
            vec![Complex::<f32>::default(); n_out_data]
        },
        |v| {},
    ));
    let lo_ch = rx_lo_ch.recv().unwrap();

    loop {
        if let Ok(payload) = rx.recv() {
            let lo_ch = if let Ok(x) = rx_lo_ch.try_recv() {
                x
            } else {
                lo_ch
            };
            if ddc.ddc(&payload.data, lo_ch) {
                let mut outdata = pool.pull_owned();
                ddc.fetch_output(&mut outdata);

                if tx.is_full() {
                    eprintln!("ddc channel full, discarding");
                    continue;
                }
                tx.send(outdata).unwrap();
            }
        }
    }
}

#[cfg(not(feature = "no_cuda"))]
pub fn pkt_ddc_stage1(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<u64>,
    ndec: usize,
    lo_ch: isize,
    fir_coeffs: &[f32],
) {
    let mut ddc = DownConverter::new(ndec, fir_coeffs);
    let n_out_data = ddc.n_out_data();

    loop {
        if let Ok(payload) = rx.recv() {
            if ddc.ddc(&payload.data, lo_ch) {
                let p = unsafe { std::mem::transmute::<*const fcomplex, u64>(ddc.0.d_outdata) };
                tx.send(p).unwrap();
            }
        }
    }
}

#[cfg(not(feature = "no_cuda"))]
pub fn pkt_ddc_stage2(
    rx: Receiver<u64>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<f32>>>>,
    ndec: usize,
    lo_ch: isize,
    fir_coeffs: &[f32],
) {
    use crate::ddc::M;

    let mut ddc = DownConverter2::new(ndec, fir_coeffs, N_PT_PER_FRAME * M / ndec);
    let n_out_data = ddc.n_out_data();

    let n_output = ddc.n_out_data();
    let pool = Arc::new(LinearObjectPool::new(
        move || vec![Complex::<f32>::default(); n_output],
        |_| {},
    ));

    loop {
        let stage1_data =
            unsafe { std::mem::transmute::<u64, *const fcomplex>(rx.recv().unwrap()) };
        ddc.ddc(stage1_data, lo_ch);
        let mut buf = pool.pull_owned();
        ddc.fetch_output(&mut buf[..]);
        tx.send(buf).unwrap();
    }
}

/*
pub fn calc_spectrum(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<f32>>>>,
    pool: Arc<LinearObjectPool<Vec<Complex<f32>>>>,
) {
    const length:usize=1024;
    let mut real_planner = RealFftPlanner::<f32>::new();
    let r2c = real_planner.plan_fft_forward(length);
    let mut fbuf=vec![0.0; N_PT_PER_FRAME];
    let n=N_PT_PER_FRAME/length;
    assert_eq!(n*length, N_PT_PER_FRAME);
    loop{
        let mut indata=rx.recv().unwrap();
        let mut channelized=;
        assert_eq!(channelized.len(), length/2+1);

        indata.data.iter().zip(fbuf.iter_mut()).for_each(|(a,b)|{
            *b=*a as f32;
        });

        fbuf.par_chunks_mut(length).zip((0..n).into_par_iter().map(|_|pool.pull_owned())).for_each(|(a,b)|{
            r2c.process(a,b).unwrap();
        });

    }
}
*/
