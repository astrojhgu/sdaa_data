use std::slice::{from_raw_parts, from_raw_parts_mut};

pub fn as_u8_slice<'a, 'b, T: Sized>(x: &'a T) -> &'b [u8]
where
    'b: 'a,
{
    unsafe { from_raw_parts((x as *const T) as *const u8, std::mem::size_of::<T>()) }
}

pub fn as_mut_u8_slice<'a, 'b, T: Sized>(x: &'a mut T) -> &'b mut [u8]
where
    'b: 'a,
{
    unsafe { from_raw_parts_mut((x as *mut T) as *mut u8, std::mem::size_of::<T>()) }
}

pub fn slice_as_u8<T: Sized>(x: &[T]) -> &[u8] {
    unsafe { from_raw_parts(x.as_ptr() as *const u8, std::mem::size_of_val(x)) }
}
