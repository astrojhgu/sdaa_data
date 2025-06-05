use std::{
    net::{SocketAddrV4, UdpSocket},
    thread::JoinHandle,
    time::Duration,
};

use crossbeam::channel::{Receiver, Sender, bounded};
use lockfree_object_pool::LinearOwnedReusable;
use num::Complex;
use sdaa_ctrl::ctrl_msg::{CmdReplySummary, CtrlMsg, send_cmd};

#[cfg(not(feature = "cuda"))]
use crate::{
    payload::Payload,
    pipeline::{DdcCmd, RecvCmd, recv_pkt},
};

#[cfg(feature = "cuda")]
use crate::{
    ddc::{N_PT_PER_FRAME, fir_coeffs_full, fir_coeffs_half},
    payload::Payload,
    pipeline::{DdcCmd, RecvCmd, pkt_ddc, recv_pkt},
};

pub struct SdrCtrl {
    pub remote_ctrl_addr: SocketAddrV4,
    pub local_ctrl_addr: SocketAddrV4,
}

impl SdrCtrl {
    pub fn send_cmd(&self, cmd: CtrlMsg) -> CmdReplySummary {
        send_cmd(
            cmd,
            &[self.remote_ctrl_addr],
            self.local_ctrl_addr,
            Some(Duration::from_secs(10)),
            1,
        )
    }

    pub fn wakeup(&self) -> CmdReplySummary {
        let cmd = CtrlMsg::PwrCtrl {
            msg_id: 0,
            op_code: 1,
        };
        self.send_cmd(cmd)
    }

    pub fn awaken_and_locked(&self) -> Option<bool> {
        let reply = self.query();
        if reply.normal_reply.len() != 1 {
            None
        } else if let (
            _,
            CtrlMsg::QueryReply {
                msg_id: _,
                fm_ver: _,
                tick_cnt1: _,
                tick_cnt2: _,
                trans_state,
                locked,
                health: _,
            },
        ) = reply.normal_reply[0]
        {
            println!("stat={trans_state:x} {locked:x}");
            Some(trans_state & 0b10 != 0 && (locked == 0x3f || locked == 0x2f))
        } else {
            None
        }
    }

    pub fn wait_until_locked(&self, timeout_sec: usize) -> bool {
        std::thread::sleep(Duration::from_secs(6));
        for _i in 0..timeout_sec {
            let reply = self.query();
            if !reply.normal_reply.is_empty()
                && let (
                    _a,
                    CtrlMsg::QueryReply {
                        msg_id: _,
                        fm_ver: _,
                        tick_cnt1: _,
                        tick_cnt2: _,
                        trans_state: _,
                        ref locked,
                        health: _,
                    },
                ) = reply.normal_reply[0]
                && (*locked == 0x3f || *locked == 0x2f)
            {
                return true;
            }

            std::thread::sleep(Duration::from_secs(1));
        }
        false
    }

    pub fn query(&self) -> CmdReplySummary {
        let cmd = CtrlMsg::Query { msg_id: 0 };
        self.send_cmd(cmd)
    }

    pub fn sync(&self) -> CmdReplySummary {
        let cmd = CtrlMsg::Sync { msg_id: 0 };
        self.send_cmd(cmd)
    }

    pub fn init(&self) -> CmdReplySummary {
        let cmd = CtrlMsg::Init {
            msg_id: 0,
            reserved_zeros: 0,
        };
        self.send_cmd(cmd)
    }

    pub fn stream_start(&self) -> CmdReplySummary {
        let cmd = CtrlMsg::StreamStart { msg_id: 0 };
        self.send_cmd(cmd)
    }

    pub fn stream_stop(&self) -> CmdReplySummary {
        println!("stopped");
        let cmd = CtrlMsg::StreamStop { msg_id: 0 };
        self.send_cmd(cmd)
    }
}

#[cfg(feature = "cuda")]
#[derive(Debug, Clone, Copy)]
pub enum SdrSmpRate {
    SmpRate240,
    SmpRate120,
}

#[cfg(feature = "cuda")]
impl SdrSmpRate {
    pub fn to_ndec(&self) -> usize {
        match self {
            SdrSmpRate::SmpRate240 => 2,
            SdrSmpRate::SmpRate120 => 4,
        }
    }

    pub fn from_ndec(ndec: usize) -> SdrSmpRate {
        match ndec {
            2 => SdrSmpRate::SmpRate240,
            4 => SdrSmpRate::SmpRate120,
            _ => panic!("invalid ndec"),
        }
    }
}

#[cfg(feature = "cuda")]
pub struct Sdr {
    rx_thread: Option<JoinHandle<()>>,
    ddc_thread: Option<JoinHandle<()>>,
    pub ctrl: SdrCtrl,
}

#[cfg(feature = "cuda")]
impl Drop for Sdr {
    fn drop(&mut self) {
        eprintln!("dropped");
        self.ctrl.stream_stop();
        let h = self.ddc_thread.take();
        eprintln!("drop1");
        if let Some(h1) = h
            && let Ok(()) = h1.join()
        {}

        eprintln!("drop2");
        let h = self.rx_thread.take();
        if let Some(h1) = h
            && let Ok(()) = h1.join()
        {}
    }
}

#[cfg(feature = "cuda")]
impl Sdr {
    #[allow(clippy::type_complexity)]
    pub fn new(
        remote_ctrl_addr: SocketAddrV4,
        local_ctrl_addr: SocketAddrV4,
        local_payload_addr: SocketAddrV4,
        smp_rate: SdrSmpRate,
    ) -> (
        Sdr,
        Receiver<LinearOwnedReusable<Vec<Complex<f32>>>>,
        Sender<DdcCmd>,
    ) {
        let payload_socket =
            UdpSocket::bind(local_payload_addr).expect("failed to bind payload socket");

        send_cmd(
            CtrlMsg::StreamStop { msg_id: 0 },
            &[remote_ctrl_addr],
            local_ctrl_addr,
            Some(Duration::from_secs(10)),
            1,
        );
        let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(8192);
        let (tx_ddc, rx_ddc) = bounded::<LinearOwnedReusable<Vec<Complex<f32>>>>(8192);
        let (tx_ddc_cmd, rx_ddc_cmd) = bounded::<DdcCmd>(32);
        let (tx_recv_cmd, rx_recv_cmd) = bounded::<RecvCmd>(32);

        tx_ddc_cmd
            .send(DdcCmd::LoCh(N_PT_PER_FRAME as isize / 4))
            .expect("failed to send loch");
        let rx_thread = std::thread::spawn(|| recv_pkt(payload_socket, tx_payload, rx_recv_cmd));
        let ddc_thread = std::thread::spawn(move || {
            let fir_coeffs = match smp_rate {
                SdrSmpRate::SmpRate240 => fir_coeffs_full(),
                SdrSmpRate::SmpRate120 => fir_coeffs_half(),
            };
            pkt_ddc(
                rx_payload,
                tx_ddc,
                smp_rate.to_ndec(),
                rx_ddc_cmd,
                tx_recv_cmd,
                &fir_coeffs,
            );
        });

        (
            Sdr {
                rx_thread: Some(rx_thread),
                ddc_thread: Some(ddc_thread),
                ctrl: SdrCtrl {
                    remote_ctrl_addr,
                    local_ctrl_addr,
                },
            },
            rx_ddc,
            tx_ddc_cmd,
        )
    }
}

pub struct RawSdr {
    rx_thread: Option<JoinHandle<()>>,
    pub ctrl: SdrCtrl,
}

impl Drop for RawSdr {
    fn drop(&mut self) {
        eprintln!("dropped");
        self.ctrl.stream_stop();
        let h = self.rx_thread.take();
        if let Some(h1) = h
            && let Ok(()) = h1.join()
        {}
    }
}

impl RawSdr {
    #[allow(clippy::type_complexity)]
    pub fn new(
        remote_ctrl_addr: SocketAddrV4,
        local_ctrl_addr: SocketAddrV4,
        local_payload_addr: SocketAddrV4,
    ) -> (
        RawSdr,
        Receiver<LinearOwnedReusable<Payload>>,
        Sender<RecvCmd>,
    ) {
        let payload_socket =
            UdpSocket::bind(local_payload_addr).expect("failed to bind payload socket");

        send_cmd(
            CtrlMsg::StreamStop { msg_id: 0 },
            &[remote_ctrl_addr],
            local_ctrl_addr,
            Some(Duration::from_secs(10)),
            1,
        );
        let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(8192);
        let (tx_recv_cmd, rx_recv_cmd) = bounded::<RecvCmd>(32);
        let rx_thread = std::thread::spawn(|| recv_pkt(payload_socket, tx_payload, rx_recv_cmd));
        (
            RawSdr {
                rx_thread: Some(rx_thread),
                ctrl: SdrCtrl {
                    remote_ctrl_addr,
                    local_ctrl_addr,
                },
            },
            rx_payload,
            tx_recv_cmd,
        )
    }
}
