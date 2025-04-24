use std::{
    net::{SocketAddrV4, UdpSocket},
    thread::JoinHandle,
    time::Duration,
};

use crossbeam::channel::{bounded, Receiver, Sender};
use lockfree_object_pool::LinearOwnedReusable;
use num::Complex;
use sdaa_ctrl::ctrl_msg::{send_cmd, CmdReplySummary, CtrlMsg};

use crate::{
    ddc::{fir_coeffs_half, N_PT_PER_FRAME},
    payload::Payload,
    pipeline::{pkt_ddc, recv_pkt, DdcCmd, RecvCmd},
};

pub struct Sdr {
    rx_thread: Option<JoinHandle<()>>,
    ddc_thread: Option<JoinHandle<()>>,
    remote_ctrl_addr: SocketAddrV4,
    local_ctrl_addr: SocketAddrV4,
}

impl Drop for Sdr {
    fn drop(&mut self) {
        eprintln!("dropped");
        self.stream_stop();
        let h = self.ddc_thread.take();
        eprintln!("drop1");
        if let Some(h1) = h {
            if let Ok(()) = h1.join() {}
        }
        eprintln!("drop2");
        let h = self.rx_thread.take();
        if let Some(h1) = h {
            if let Ok(()) = h1.join() {}
        }
    }
}

impl Sdr {
    #[allow(clippy::type_complexity)]
    pub fn new(
        remote_ctrl_addr: SocketAddrV4,
        local_ctrl_addr: SocketAddrV4,
        local_payload_addr: SocketAddrV4,
    ) -> (
        Sdr,
        Receiver<LinearOwnedReusable<Vec<Complex<f32>>>>,
        Sender<DdcCmd>,
    ) {
        let payload_socket = UdpSocket::bind(local_payload_addr).unwrap();

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
            .unwrap();
        let rx_thread = std::thread::spawn(|| recv_pkt(payload_socket, tx_payload, rx_recv_cmd));
        let ddc_thread = std::thread::spawn(move || {
            let fir_coeffs = fir_coeffs_half();
            pkt_ddc(rx_payload, tx_ddc, 4, rx_ddc_cmd,tx_recv_cmd, &fir_coeffs);
        });

        (
            Sdr {
                rx_thread: Some(rx_thread),
                ddc_thread: Some(ddc_thread),
                remote_ctrl_addr,
                local_ctrl_addr,
            },
            rx_ddc,
            tx_ddc_cmd,
        )
    }

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

    pub fn wait_until_locked(&self, timeout_sec: usize) -> bool {
        std::thread::sleep(Duration::from_secs(5));
        for _i in 0..timeout_sec {
            let reply = self.query();
            if !reply.normal_reply.is_empty() {
                if let (
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
                {
                    if *locked == 0x3f || *locked == 0x2f {
                        return true;
                    }
                }
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
