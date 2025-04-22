#![feature(portable_simd)]

pub mod fir;
pub mod payload;
pub mod pipeline;
pub mod utils;

#[cfg(feature = "cuda")]
pub mod bindings;

#[cfg(feature = "cuda")]
pub mod ddc;

#[cfg(feature = "cuda")]
pub mod c_interface;

#[cfg(feature = "cuda")]
pub mod sdr;


pub type RawType = i16;
pub type Ftype = f32;
pub const RAW_SAMP_RATE: usize = 480_000_000;
