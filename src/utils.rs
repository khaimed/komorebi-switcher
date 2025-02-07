use windows::Win32::Foundation::*;

/// Implementation of RGB macro
#[allow(non_snake_case)]
#[inline]
pub const fn RGB(r: u32, g: u32, b: u32) -> COLORREF {
    COLORREF(r | g << 8 | b << 16)
}
