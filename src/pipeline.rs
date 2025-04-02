use std::default;
use std::time::{Duration, Instant};
use std::{net::UdpSocket, sync::Arc};

use chrono::Local;
use crossbeam::channel::{Receiver, Sender};
use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use rayon::{
    iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;

#[cfg(not(feature = "no_cuda"))]
use crate::ddc::DownConverter;

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
            eprint!(".");
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
        }

        while let Some(ref mut c) = next_cnt {
            //let current_cnt = c + 1;
            if *c >= payload.pkt_cnt {
                *c = payload.pkt_cnt + 1;
                tx.send(payload).unwrap();
                break;
            }

            ndropped += 1;

            let mut payload1 = pool.pull_owned();
            payload1.copy_header(&payload);
            payload1.pkt_cnt = *c;
            tx.send(payload1).unwrap();
            *c += 1;
        }
    }
}


#[cfg(not(feature="no_cuda"))]
pub fn pkt_ddc(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<f32>>>>,
    ndec: usize,
    lo_ch: isize, 
    fir_coeffs: &[f32],
) {
    let mut ddc = DownConverter::new(ndec, fir_coeffs);

    let pool: Arc<LinearObjectPool<Vec<Complex<f32>>>> = Arc::new(LinearObjectPool::new(
        move || {
            eprint!(".");
            vec![Complex::<f32>::default(); ddc.n_out_data]
        },
        |v| {},
    ));

    loop {
        loop {
            let payload = rx.recv().unwrap();
            if ddc.ddc(&payload.data,lo_ch) {
                let mut outdata = pool.pull_owned();
                ddc.fetch_output(&mut outdata);
                tx.send(outdata).unwrap();
                break;
            }
        }
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
