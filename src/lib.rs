#![feature(portable_simd)]

pub mod fir;
pub mod payload;
pub mod pipeline;
pub mod utils;

#[cfg(not(feature = "no_cuda"))]
pub mod bindings;

#[cfg(not(feature = "no_cuda"))]
pub mod ddc;

#[cfg(not(feature = "no_cuda"))]
pub mod c_interface;


pub type RawType = i16;
pub type Ftype = f32;
pub const RAW_SAMP_RATE: usize = 480_000_000;
