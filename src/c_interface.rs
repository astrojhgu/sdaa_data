#![allow(static_mut_refs)]
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_ushort, CStr},
    net::UdpSocket,
    time::Duration,
};

use crossbeam::channel::{bounded, Receiver, Sender};
use lockfree_object_pool::LinearOwnedReusable;
use num::Complex;

use crate::{
    ddc::{fir_coeffs2, M, N_PT_PER_FRAME},
    payload::Payload,
    pipeline::{pkt_ddc, recv_pkt},
};

use sdaa_ctrl::ctrl_msg::{send_cmd, CtrlMsg};

static mut DDC_RX_HANDLER: BTreeMap<u32, Receiver<LinearOwnedReusable<Vec<Complex<f32>>>>> =
    BTreeMap::new();

static mut LO_RX_HANDLER: BTreeMap<u32, Sender<isize>> = BTreeMap::new();

const NDEC: usize = 8;

fn next_available_handle() -> u32 {
    if unsafe { DDC_RX_HANDLER.is_empty() } {
        1
    } else {
        unsafe { DDC_RX_HANDLER.keys() }.cloned().max().unwrap() + 1
    }
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn start_data_receiving(
    ctrl_ip: *const c_char,
    data_ip: *const c_char,
    data_port: c_ushort,
    lo_ch: isize,
) -> u32 {
    let c_str = CStr::from_ptr(ctrl_ip);
    let ctrl_addr = vec![if let Ok(s) = c_str.to_str() {
        format!("{}:3000", s)
    } else {
        return 0;
    }];

    let c_str = CStr::from_ptr(data_ip);
    let data_addr = if let Ok(s) = c_str.to_str() {
        format!("{}:{}", s, data_port)
    } else {
        return 0;
    };

    let local_addr = format!("0.0.0.0:{}", 3001);

    let cmd = CtrlMsg::StreamStop { msg_id: 0 };
    send_cmd(
        cmd,
        &ctrl_addr,
        &local_addr,
        Some(Duration::from_secs(10)),
        10,
    );

    let socket = UdpSocket::bind(data_addr).unwrap();

    let (tx_payload, rx_payload) = bounded::<LinearOwnedReusable<Payload>>(8192);
    let (tx_ddc, rx_ddc) = bounded::<LinearOwnedReusable<Vec<Complex<f32>>>>(8192);
    let (tx_lo_ch, rx_lo_ch) = bounded::<isize>(32);

    tx_lo_ch.send(lo_ch).unwrap();

    std::thread::spawn(|| recv_pkt(socket, tx_payload));
    std::thread::spawn(move || {
        let fir_coeffs = fir_coeffs2();
        pkt_ddc(rx_payload, tx_ddc, NDEC, rx_lo_ch, &fir_coeffs);
    });

    let handle = next_available_handle();

    unsafe { DDC_RX_HANDLER.insert(handle, rx_ddc) };
    unsafe { LO_RX_HANDLER.insert(handle, tx_lo_ch) };

    let cmd = CtrlMsg::StreamStart { msg_id: 0 };
    send_cmd(
        cmd,
        &ctrl_addr,
        &local_addr,
        Some(Duration::from_secs(10)),
        10,
    );

    handle
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn stop_data_receiving(
    ctrl_ip: *const c_char,
    data_ip: *const c_char,
    data_port: c_ushort,
) -> u32 {
    let c_str = CStr::from_ptr(ctrl_ip);
    let ctrl_addr = vec![if let Ok(s) = c_str.to_str() {
        format!("{}:3000", s)
    } else {
        return 0;
    }];

    let c_str = CStr::from_ptr(data_ip);
    let _data_addr = if let Ok(s) = c_str.to_str() {
        format!("{}:{}", s, data_port)
    } else {
        return 0;
    };

    let local_addr = format!("0.0.0.0:{}", 3001);

    let cmd = CtrlMsg::StreamStop { msg_id: 0 };
    send_cmd(
        cmd,
        &ctrl_addr,
        &local_addr,
        Some(Duration::from_secs(10)),
        10,
    );
    0
}

/// # Safety
///
/// This function should not be called before the horsemen are ready.
#[no_mangle]
pub unsafe extern "C" fn get_mtu() -> usize {
    N_PT_PER_FRAME * M / NDEC
}
