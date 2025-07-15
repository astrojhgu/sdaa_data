use std::net::SocketAddrV4;
use std::time::{Duration, Instant};
use std::{
    net::{Ipv4Addr, UdpSocket},
    ops::Deref,
    sync::Arc,
};

use chrono::Local;
use crossbeam::channel::{Receiver, Sender};
use lockfree_object_pool::{LinearObjectPool, LinearOwnedReusable};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

#[cfg(feature = "cuda")]
use crate::ddc::DownConverter;

use crate::{
    payload::{N_PT_PER_FRAME, Payload},
    utils::as_mut_u8_slice,
};

pub struct MaybeMulticastReceiver {
    socket: UdpSocket,
    group_and_iface: Option<(Ipv4Addr, Ipv4Addr)>, // (group, iface)
}

impl MaybeMulticastReceiver {
    pub fn new(
        bind_addr: SocketAddrV4,
        group_and_iface: Option<(Ipv4Addr, Ipv4Addr)>,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;

        if let Some((group, iface)) = group_and_iface {
            socket.join_multicast_v4(&group, &iface)?;
        }

        Ok(Self {
            socket,
            group_and_iface,
        })
    }
}

impl Drop for MaybeMulticastReceiver {
    fn drop(&mut self) {
        if let Some((group, iface)) = self.group_and_iface {
            let _ = self.socket.leave_multicast_v4(&group, &iface);
            println!("Left multicast group {} on interface {}", group, iface);
        }
    }
}

impl Deref for MaybeMulticastReceiver {
    type Target = UdpSocket;
    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

impl From<UdpSocket> for MaybeMulticastReceiver {
    fn from(socket: UdpSocket) -> Self {
        Self {
            socket,
            group_and_iface: None,
        }
    }
}

pub enum RecvCmd {
    Destroy,
}

pub fn fake_dev(tx_payload: Sender<LinearOwnedReusable<Payload>>, rx_cmd: Receiver<RecvCmd>) {
    let mut last_print_time = Instant::now();
    let t0 = Instant::now();
    let print_interval = Duration::from_secs(2);

    let pool: Arc<LinearObjectPool<Payload>> = Arc::new(LinearObjectPool::new(
        move || {
            //eprint!("o");
            Payload::default()
        },
        |v| {
            v.pkt_cnt = 0;
            v.data.fill(0);
        },
    ));
    //socket.set_nonblocking(true).unwrap();
    for pkt_cnt in 0.. {
        if !rx_cmd.is_empty() {
            match rx_cmd.recv().expect("failed to recv cmd") {
                RecvCmd::Destroy => break,
            }
        }
        let mut payload = pool.pull_owned();
        payload.pkt_cnt = pkt_cnt;

        let now = Instant::now();

        if payload.pkt_cnt == 0 {
            let local_time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            println!();
            println!("==================================");
            println!("start time:{local_time}");
            println!("==================================");
        } else if now.duration_since(last_print_time) >= print_interval {
            let dt = now.duration_since(t0).as_secs_f64();
            let npkts = pkt_cnt as usize;
            let nsamp = npkts * N_PT_PER_FRAME;
            let smp_rate = nsamp as f64 / dt;
            println!("smp_rate: {} MSps q={}", smp_rate / 1e6, tx_payload.len());
            last_print_time = now;
        }

        if tx_payload.send(payload).is_err() {
            return;
        }
    }
}

pub fn recv_pkt(
    socket: MaybeMulticastReceiver,
    tx_payload: Sender<LinearOwnedReusable<Payload>>,
    rx_cmd: Receiver<RecvCmd>,
) {
    let mut last_print_time = Instant::now();
    let print_interval = Duration::from_secs(2);

    let mut next_cnt = None;
    let mut ndropped = 0;
    let mut nreceived = 0;
    let pool: Arc<LinearObjectPool<Payload>> = Arc::new(LinearObjectPool::new(
        move || {
            //eprint!("o");
            Payload::default()
        },
        |v| {
            v.pkt_cnt = 0;
            v.data.fill(0);
        },
    ));
    //socket.set_nonblocking(true).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("failed to set timeout");
    loop {
        if !rx_cmd.is_empty() {
            match rx_cmd.recv().expect("failed to recv cmd") {
                RecvCmd::Destroy => break,
            }
        }
        let mut payload = pool.pull_owned();
        let buf = as_mut_u8_slice(&mut payload as &mut Payload);
        match socket.recv_from(buf) {
            Ok((s, _a)) => {
                if s != std::mem::size_of::<Payload>() {
                    continue;
                }
            }
            _ => continue,
        }

        let now = Instant::now();

        if now.duration_since(last_print_time) >= print_interval {
            let local_time = Local::now().format("%Y-%m-%d %H:%M:%S");
            println!(
                "{local_time} {ndropped} pkts dropped q={} ratio<{:e}",
                tx_payload.len(),
                (1 + ndropped) as f64 / nreceived as f64
            );
            last_print_time = now;
        }

        if next_cnt.is_none() {
            next_cnt = Some(payload.pkt_cnt);
            ndropped = 0;
        }

        if payload.pkt_cnt == 0 {
            ndropped = 0;
            nreceived = 0;
            let local_time = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            println!();
            println!("==================================");
            println!("start time:{local_time}");
            println!("==================================");
        }

        while let Some(ref mut c) = next_cnt {
            //let current_cnt = c + 1;
            if *c >= payload.pkt_cnt {
                //actually = is sufficient.
                *c = payload.pkt_cnt + 1;
                if tx_payload.is_full() {
                    //eprint!("O");
                    if !rx_cmd.is_empty() {
                        match rx_cmd.recv().expect("failed to recv cmd") {
                            RecvCmd::Destroy => return,
                        }
                    }
                    continue;
                }
                nreceived += 1;
                if let Ok(()) = tx_payload.send(payload) {
                    break;
                } else {
                    return;
                }
            }

            ndropped += 1;

            let mut payload1 = pool.pull_owned();
            payload1.copy_header(&payload);
            payload1.pkt_cnt = *c;
            if tx_payload.is_full() {
                //eprint!("O");
                if !rx_cmd.is_empty() {
                    match rx_cmd.recv().expect("failed to recv cmd") {
                        RecvCmd::Destroy => return,
                    }
                }
                continue;
            }
            nreceived += 1;
            if tx_payload.send(payload1).is_err() {
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
            //eprint!(".");
            vec![Complex::default(); nbuf / 2]
        },
        |_v| {},
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
            buffer
                .chunks(nch)
                .step_by(2)
                .zip(result.chunks_mut(nch))
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

#[cfg(feature = "cuda")]
pub fn pkt_wf(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<f32>>>,
    nch: usize,
    nbatch: usize,
    nint: usize,
) {
    assert_eq!(nbatch % nint, 0);
    use crate::cuwf::WfResource;
    let mut wf = WfResource::new(nch, nbatch, nint);
    let nbuf = nch * nbatch / nint;

    let pool: Arc<LinearObjectPool<Vec<f32>>> = Arc::new(LinearObjectPool::new(
        move || {
            //eprint!(".");
            vec![0_f32; nbuf]
        },
        |_v| {},
    ));
    let mut result = pool.pull_owned();
    while let Ok(payload) = rx.recv() {
        if wf.process(&payload.data, result.as_mut_slice()) {
            if tx.is_full() {
                eprintln!("waterfall channel full, discarding");
                continue;
            }
            if tx.send(result).is_err() {
                break;
            }
            result = pool.pull_owned();
        }
    }
}

pub fn pkt_integrate(
    rx: Receiver<LinearOwnedReusable<Vec<Complex<f32>>>>,
    tx: Sender<LinearOwnedReusable<Vec<f32>>>,
    nch: usize,
    nint: usize,
) {
    let pool: Arc<LinearObjectPool<Vec<f32>>> = Arc::new(LinearObjectPool::new(
        move || {
            //eprint!(".");
            vec![0.0; nch]
        },
        |v| {
            v.fill(0.0);
        },
    ));

    let mut result = pool.pull_owned();
    let mut add_cnt = 0;
    while let Ok(x) = rx.recv() {
        let n = x.len();
        assert!(n % nch == 0);
        for x1 in x.chunks(nch) {
            result.iter_mut().zip(x1).for_each(|(a, b)| {
                *a += b.norm_sqr();
            });
            add_cnt += 1;
            if add_cnt == nint {
                add_cnt = 0;
                if tx.send(result).is_err() {
                    return;
                }
                result = pool.pull_owned();
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DdcCmd {
    LoCh(isize),
    Destroy,
}

#[cfg(feature = "cuda")]
pub fn pkt_ddc(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<LinearOwnedReusable<Vec<Complex<f32>>>>,
    ndec: usize,
    rx_ddc_cmd: Receiver<DdcCmd>,
    tx_recv_cmd: Sender<RecvCmd>,
    fir_coeffs: &[f32],
) {
    let mut ddc = DownConverter::new(ndec, fir_coeffs);
    let n_out_data = ddc.n_out_data();
    let pool: Arc<LinearObjectPool<Vec<Complex<f32>>>> = Arc::new(LinearObjectPool::new(
        move || {
            //eprint!(".");
            vec![Complex::<f32>::default(); n_out_data]
        },
        |_v| {},
    ));

    let mut lo_ch = if let DdcCmd::LoCh(c) = rx_ddc_cmd.recv().expect("failed to recv cmd") {
        c
    } else {
        N_PT_PER_FRAME as isize / 4
    };

    loop {
        if !rx_ddc_cmd.is_empty() {
            if let Ok(x) = rx_ddc_cmd.recv() {
                match x {
                    DdcCmd::LoCh(c) => {
                        lo_ch = c;
                    }
                    DdcCmd::Destroy => {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        if let Ok(payload) = rx.recv_timeout(Duration::from_secs(1))
            && ddc.ddc(&payload.data, lo_ch)
        {
            let mut outdata = pool.pull_owned();
            ddc.fetch_output(&mut outdata);

            if tx.is_full() {
                eprintln!("ddc channel full, discarding");
                continue;
            }
            if tx.send_timeout(outdata, Duration::from_secs(1)).is_err() {
                break;
            }
        }
    }
    drop(rx);
    tx_recv_cmd
        .send(RecvCmd::Destroy)
        .expect("failed to send cmd");
}

/*
#[cfg(feature = "cuda")]
pub fn pkt_ddc_stage1(
    rx: Receiver<LinearOwnedReusable<Payload>>,
    tx: Sender<u64>,
    ndec: usize,
    lo_ch: isize,
    fir_coeffs: &[f32],
) {
    let mut ddc = DownConverter::new(ndec, fir_coeffs);
    let _n_out_data = ddc.n_out_data();

    loop {
        if let Ok(payload) = rx.recv() {
            if ddc.ddc(&payload.data, lo_ch) {
                let p = ddc.0.d_outdata as *const fcomplex as u64;
                tx.send(p).unwrap();
            }
        }
    }
}


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
