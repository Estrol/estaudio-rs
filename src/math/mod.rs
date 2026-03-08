#![allow(dead_code)]

trait SimdDiv<T: Copy> {
    fn simd_div(array: &mut [T], value: &[T]);
}

struct DivUtil;

macro_rules! impl_scalar_div {
    ($ty:ty) => {
        impl SimdDiv<$ty> for DivUtil {
            #[inline(always)]
            fn simd_div(array: &mut [$ty], value: &[$ty]) {
                if array.len() != value.len() {
                    panic!("Input and value arrays must have the same length");
                }

                // unroll 4
                let len = array.len();
                let mut i = 0;
                while i + 4 <= len {
                    array[i] /= value[i];
                    array[i + 1] /= value[i + 1];
                    array[i + 2] /= value[i + 2];
                    array[i + 3] /= value[i + 3];
                    i += 4;
                }

                // handle remaining elements
                while i < len {
                    array[i] /= value[i];
                    i += 1;
                }
            }
        }
    };
}

macro_rules! impl_simd_div {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdDiv<$ty> for DivUtil {
            #[inline(always)]
            fn simd_div(array: &mut [$ty], value: &[$ty]) {
                if array.len() != value.len() {
                    panic!("Input and value arrays must have the same length");
                }

                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE).zip(value.chunks(CHUNK_SIZE)) {
                    if chunk.0.len() == CHUNK_SIZE {
                        unsafe {
                            let src_ptr = chunk.0.as_ptr() as *const $wide_ty;
                            let value_ptr = chunk.1.as_ptr() as *const $wide_ty;

                            let dst_ptr = chunk.0.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(src_ptr);
                            let wide_value = std::ptr::read_unaligned(value_ptr);

                            let result = wide_chunk / wide_value;
                            std::ptr::write_unaligned(dst_ptr, result);
                        }
                    } else {
                        for i in 0..chunk.0.len() {
                            chunk.0[i] /= chunk.1[i];
                        }
                    }
                }
            }
        }
    };
}

impl_scalar_div!(u32);
impl_scalar_div!(i32);
impl_scalar_div!(i16);
impl_scalar_div!(u16);
impl_simd_div!(f32, wide::f32x4);
impl_simd_div!(f64, wide::f64x2);

trait SimdMul<T: Copy> {
    fn simd_mul(array: &mut [T], value: &[T]);
}

struct MulUtil;

macro_rules! impl_simd_mul {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdMul<$ty> for MulUtil {
            #[inline(always)]
            fn simd_mul(array: &mut [$ty], value: &[$ty]) {
                if array.len() != value.len() {
                    panic!("Input and value arrays must have the same length");
                }

                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE).zip(value.chunks(CHUNK_SIZE)) {
                    if chunk.0.len() == CHUNK_SIZE {
                        unsafe {
                            let src_ptr = chunk.0.as_ptr() as *const $wide_ty;
                            let value_ptr = chunk.1.as_ptr() as *const $wide_ty;

                            let dst_ptr = chunk.0.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(src_ptr);
                            let wide_value = std::ptr::read_unaligned(value_ptr);

                            let result = wide_chunk * wide_value;
                            std::ptr::write_unaligned(dst_ptr, result);
                        }
                    } else {
                        for i in 0..chunk.0.len() {
                            chunk.0[i] *= chunk.1[i];
                        }
                    }
                }
            }
        }
    };
}

impl_simd_mul!(f32, wide::f32x4);
impl_simd_mul!(f64, wide::f64x2);
impl_simd_mul!(u32, wide::u32x4);
impl_simd_mul!(i32, wide::i32x4);
impl_simd_mul!(i16, wide::i16x8);
impl_simd_mul!(u16, wide::u16x8);

trait SimdAdd<T: Copy> {
    fn simd_add(array: &mut [T], value: &[T]);
}

struct AddUtil;

macro_rules! impl_simd_add {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdAdd<$ty> for AddUtil {
            #[inline(always)]
            fn simd_add(array: &mut [$ty], value: &[$ty]) {
                if array.len() != value.len() {
                    panic!("Input and value arrays must have the same length");
                }

                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE).zip(value.chunks(CHUNK_SIZE)) {
                    if chunk.0.len() == CHUNK_SIZE {
                        unsafe {
                            let src_ptr = chunk.0.as_ptr() as *const $wide_ty;
                            let value_ptr = chunk.1.as_ptr() as *const $wide_ty;

                            let dst_ptr = chunk.0.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(src_ptr);
                            let wide_value = std::ptr::read_unaligned(value_ptr);

                            let result = wide_chunk + wide_value;
                            std::ptr::write_unaligned(dst_ptr, result);
                        }
                    } else {
                        for i in 0..chunk.0.len() {
                            chunk.0[i] += chunk.1[i];
                        }
                    }
                }
            }
        }
    };
}

impl_simd_add!(f32, wide::f32x4);
impl_simd_add!(f64, wide::f64x2);
impl_simd_add!(u32, wide::u32x4);
impl_simd_add!(i32, wide::i32x4);
impl_simd_add!(i16, wide::i16x8);
impl_simd_add!(u16, wide::u16x8);

trait SimdSub<T: Copy> {
    fn simd_sub(array: &mut [T], value: &[T]);
}

struct SubUtil;

macro_rules! impl_simd_sub {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdSub<$ty> for SubUtil {
            #[inline(always)]
            fn simd_sub(array: &mut [$ty], value: &[$ty]) {
                if array.len() != value.len() {
                    panic!("Input and value arrays must have the same length");
                }

                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE).zip(value.chunks(CHUNK_SIZE)) {
                    if chunk.0.len() == CHUNK_SIZE {
                        unsafe {
                            let src_ptr = chunk.0.as_ptr() as *const $wide_ty;
                            let value_ptr = chunk.1.as_ptr() as *const $wide_ty;

                            let dst_ptr = chunk.0.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(src_ptr);
                            let wide_value = std::ptr::read_unaligned(value_ptr);

                            let result = wide_chunk - wide_value;
                            std::ptr::write_unaligned(dst_ptr, result);
                        }
                    } else {
                        for i in 0..chunk.0.len() {
                            chunk.0[i] -= chunk.1[i];
                        }
                    }
                }
            }
        }
    };
}

impl_simd_sub!(f32, wide::f32x4);
impl_simd_sub!(f64, wide::f64x2);
impl_simd_sub!(u32, wide::u32x4);
impl_simd_sub!(i32, wide::i32x4);
impl_simd_sub!(i16, wide::i16x8);
impl_simd_sub!(u16, wide::u16x8);

trait SimdClamp<T: Copy> {
    fn simd_clamp(array: &mut [T], min: T, max: T);
}

struct ClampUtil;

macro_rules! impl_simd_clamp {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdClamp<$ty> for ClampUtil {
            #[inline(always)]
            fn simd_clamp(array: &mut [$ty], min: $ty, max: $ty) {
                let wide_min = <$wide_ty>::splat(min);
                let wide_max = <$wide_ty>::splat(max);
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE) {
                    if chunk.len() == CHUNK_SIZE {
                        unsafe {
                            let ptr = chunk.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(ptr);
                            let clamped = wide_chunk.min(wide_max).max(wide_min);
                            std::ptr::write_unaligned(ptr, clamped);
                        }
                    } else {
                        for i in 0..chunk.len() {
                            chunk[i] = chunk[i].min(max).max(min);
                        }
                    }
                }
            }
        }
    };
}

impl_simd_clamp!(f32, wide::f32x4);
impl_simd_clamp!(f64, wide::f64x2);
impl_simd_clamp!(u32, wide::u32x4);
impl_simd_clamp!(i32, wide::i32x4);
impl_simd_clamp!(i16, wide::i16x8);
impl_simd_clamp!(u16, wide::u16x8);

trait SimdMin<T: Copy> {
    fn simd_min(array: &mut [T], value: T);
}

struct MinUtil;

macro_rules! impl_simd_min {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdMin<$ty> for MinUtil {
            #[inline(always)]
            fn simd_min(array: &mut [$ty], value: $ty) {
                let wide_value = <$wide_ty>::splat(value);
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE) {
                    if chunk.len() == CHUNK_SIZE {
                        unsafe {
                            let ptr = chunk.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(ptr);
                            let result = wide_chunk.min(wide_value);
                            std::ptr::write_unaligned(ptr, result);
                        }
                    } else {
                        for i in 0..chunk.len() {
                            chunk[i] = chunk[i].min(value);
                        }
                    }
                }
            }
        }
    };
}

impl_simd_min!(f32, wide::f32x4);
impl_simd_min!(f64, wide::f64x2);
impl_simd_min!(u32, wide::u32x4);
impl_simd_min!(i32, wide::i32x4);
impl_simd_min!(i16, wide::i16x8);
impl_simd_min!(u16, wide::u16x8);

trait SimdMax<T: Copy> {
    fn simd_max(array: &mut [T], value: T);
}

struct MaxUtil;

macro_rules! impl_simd_max {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdMax<$ty> for MaxUtil {
            #[inline(always)]
            fn simd_max(array: &mut [$ty], value: $ty) {
                let wide_value = <$wide_ty>::splat(value);
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE) {
                    if chunk.len() == CHUNK_SIZE {
                        unsafe {
                            let ptr = chunk.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(ptr);
                            let result = wide_chunk.max(wide_value);
                            std::ptr::write_unaligned(ptr, result);
                        }
                    } else {
                        for i in 0..chunk.len() {
                            chunk[i] = chunk[i].max(value);
                        }
                    }
                }
            }
        }
    };
}

impl_simd_max!(f32, wide::f32x4);
impl_simd_max!(f64, wide::f64x2);
impl_simd_max!(u32, wide::u32x4);
impl_simd_max!(i32, wide::i32x4);
impl_simd_max!(i16, wide::i16x8);
impl_simd_max!(u16, wide::u16x8);

trait SimdCopy<T: Copy> {
    fn simd_copy(src: &[T], dst: &mut [T]);
}

struct CopyUtil;

macro_rules! impl_simd_copy {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdCopy<$ty> for CopyUtil {
            #[inline(always)]
            fn simd_copy(src: &[$ty], dst: &mut [$ty]) {
                assert_eq!(src.len(), dst.len());
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for (chunk_src, chunk_dst) in src.chunks(CHUNK_SIZE).zip(dst.chunks_mut(CHUNK_SIZE))
                {
                    if chunk_src.len() == CHUNK_SIZE {
                        unsafe {
                            let src_ptr = chunk_src.as_ptr() as *const $wide_ty;
                            let dst_ptr = chunk_dst.as_mut_ptr() as *mut $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(src_ptr);
                            std::ptr::write_unaligned(dst_ptr, wide_chunk);
                        }
                    } else {
                        chunk_dst.copy_from_slice(chunk_src);
                    }
                }
            }
        }
    };
}

impl_simd_copy!(f32, wide::f32x4);
impl_simd_copy!(f64, wide::f64x2);
impl_simd_copy!(u32, wide::u32x4);
impl_simd_copy!(i32, wide::i32x4);
impl_simd_copy!(i16, wide::i16x8);
impl_simd_copy!(u16, wide::u16x8);

trait SimdSet<T: Copy> {
    fn simd_set(array: &mut [T], value: T);
}

struct SetUtil;

macro_rules! impl_simd_set {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdSet<$ty> for SetUtil {
            #[inline(always)]
            fn simd_set(array: &mut [$ty], value: $ty) {
                let wide_value = <$wide_ty>::splat(value);
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks_mut(CHUNK_SIZE) {
                    unsafe {
                        let ptr = chunk.as_mut_ptr() as *mut $wide_ty;
                        if chunk.len() == CHUNK_SIZE {
                            *(ptr) = wide_value;
                        } else {
                            for i in 0..chunk.len() {
                                chunk[i] = value;
                            }
                        }
                    }
                }
            }
        }
    };
}

impl_simd_set!(f32, wide::f32x4);
impl_simd_set!(f64, wide::f64x2);
impl_simd_set!(u32, wide::u32x4);
impl_simd_set!(i32, wide::i32x4);
impl_simd_set!(i16, wide::i16x8);
impl_simd_set!(u16, wide::u16x8);

trait SimdAny<T: Copy> {
    fn simd_any(array: &[T], value: T) -> bool;
}

struct AnyUtil;

macro_rules! impl_simd_any {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdAny<$ty> for AnyUtil {
            #[inline(always)]
            fn simd_any(array: &[$ty], value: $ty) -> bool {
                let wide_value = <$wide_ty>::splat(value);
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks(CHUNK_SIZE) {
                    if chunk.len() == CHUNK_SIZE {
                        unsafe {
                            let ptr = chunk.as_ptr() as *const $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(ptr);
                            if wide_chunk == wide_value {
                                return true;
                            }
                        }
                    } else {
                        for i in 0..chunk.len() {
                            if chunk[i] == value {
                                return true;
                            }
                        }
                    }
                }

                false
            }
        }
    };
}

impl_simd_any!(f32, wide::f32x4);
impl_simd_any!(f64, wide::f64x2);
impl_simd_any!(u32, wide::u32x4);
impl_simd_any!(i32, wide::i32x4);
impl_simd_any!(i16, wide::i16x8);
impl_simd_any!(u16, wide::u16x8);

trait SimdNotAny<T: Copy> {
    fn simd_not_any(array: &[T], value: T) -> bool;
}

struct NotAnyUtil;

macro_rules! impl_simd_not_any {
    ($ty:ty, $wide_ty:ty) => {
        impl SimdNotAny<$ty> for NotAnyUtil {
            #[inline(always)]
            fn simd_not_any(array: &[$ty], value: $ty) -> bool {
                let wide_value = <$wide_ty>::splat(value);
                const CHUNK_SIZE: usize =
                    std::mem::size_of::<$wide_ty>() / std::mem::size_of::<$ty>();

                for chunk in array.chunks(CHUNK_SIZE) {
                    if chunk.len() == CHUNK_SIZE {
                        unsafe {
                            let ptr = chunk.as_ptr() as *const $wide_ty;
                            let wide_chunk = std::ptr::read_unaligned(ptr);
                            if wide_chunk != wide_value {
                                return false;
                            }
                        }
                    } else {
                        for i in 0..chunk.len() {
                            if chunk[i] != value {
                                return false;
                            }
                        }
                    }
                }

                true
            }
        }
    };
}

impl_simd_not_any!(f32, wide::f32x4);
impl_simd_not_any!(f64, wide::f64x2);
impl_simd_not_any!(u32, wide::u32x4);
impl_simd_not_any!(i32, wide::i32x4);
impl_simd_not_any!(i16, wide::i16x8);
impl_simd_not_any!(u16, wide::u16x8);

/// Helper trait for overloading math utility functions for different types.
pub trait MathUtilsTrait<T: Copy> {
    fn simd_div(array: &mut [T], value: &[T]);
    fn simd_mul(array: &mut [T], value: &[T]);
    fn simd_add(array: &mut [T], value: &[T]);
    fn simd_sub(array: &mut [T], value: &[T]);
    fn simd_clamp(array: &mut [T], min: T, max: T);
    fn simd_min(array: &mut [T], value: T);
    fn simd_max(array: &mut [T], value: T);
    fn simd_copy(src: &[T], dst: &mut [T]);
    fn simd_set(array: &mut [T], value: T);
    fn simd_any(array: &[T], value: T) -> bool;
    fn simd_not_any(array: &[T], value: T) -> bool;
}

/// A utility struct for performing SIMD operations on audio data.
pub struct MathUtils<T: Copy>(std::marker::PhantomData<T>);

macro_rules! impl_math_utils {
    ($ty:ty, $wide_ty:ty) => {
        impl MathUtilsTrait<$ty> for MathUtils<$ty> {
            #[inline(always)]
            fn simd_div(array: &mut [$ty], value: &[$ty]) {
                DivUtil::simd_div(array, value);
            }

            #[inline(always)]
            fn simd_mul(array: &mut [$ty], value: &[$ty]) {
                MulUtil::simd_mul(array, value);
            }

            #[inline(always)]
            fn simd_add(array: &mut [$ty], value: &[$ty]) {
                AddUtil::simd_add(array, value);
            }

            #[inline(always)]
            fn simd_sub(array: &mut [$ty], value: &[$ty]) {
                SubUtil::simd_sub(array, value);
            }

            #[inline(always)]
            fn simd_clamp(array: &mut [$ty], min: $ty, max: $ty) {
                ClampUtil::simd_clamp(array, min, max);
            }

            #[inline(always)]
            fn simd_min(array: &mut [$ty], value: $ty) {
                MinUtil::simd_min(array, value);
            }

            #[inline(always)]
            fn simd_max(array: &mut [$ty], value: $ty) {
                MaxUtil::simd_max(array, value);
            }

            #[inline(always)]
            fn simd_copy(src: &[$ty], dst: &mut [$ty]) {
                CopyUtil::simd_copy(src, dst);
            }

            #[inline(always)]
            fn simd_set(array: &mut [$ty], value: $ty) {
                SetUtil::simd_set(array, value);
            }

            #[inline(always)]
            fn simd_any(array: &[$ty], value: $ty) -> bool {
                AnyUtil::simd_any(array, value)
            }

            #[inline(always)]
            fn simd_not_any(array: &[$ty], value: $ty) -> bool {
                NotAnyUtil::simd_not_any(array, value)
            }
        }
    };
}

impl_math_utils!(f32, wide::f32x4);
impl_math_utils!(f64, wide::f64x2);
impl_math_utils!(u32, wide::u32x4);
impl_math_utils!(i32, wide::i32x4);
impl_math_utils!(i16, wide::i16x8);
impl_math_utils!(u16, wide::u16x8);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simd_div() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        let value = [1.0f32, 2.0, 3.0, 4.0];
        MathUtils::<f32>::simd_div(&mut data, &value);
        assert_eq!(data, [1.0f32, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_simd_mul() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        let value = [2.0f32, 2.0, 2.0, 2.0];
        MathUtils::<f32>::simd_mul(&mut data, &value);
        assert_eq!(data, [2.0f32, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn test_simd_add() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        let value = [1.0f32, 1.0, 1.0, 1.0];
        MathUtils::<f32>::simd_add(&mut data, &value);
        assert_eq!(data, [2.0f32, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_simd_sub() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        let value = [1.0f32, 1.0, 1.0, 1.0];
        MathUtils::<f32>::simd_sub(&mut data, &value);
        assert_eq!(data, [0.0f32, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_simd_clamp() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        MathUtils::<f32>::simd_clamp(&mut data, 2.0, 3.0);
        assert_eq!(data, [2.0f32, 2.0, 3.0, 3.0]);
    }

    #[test]
    fn test_simd_min() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        MathUtils::<f32>::simd_min(&mut data, 2.5);
        assert_eq!(data, [1.0f32, 2.0, 2.5, 2.5]);
    }

    #[test]
    fn test_simd_max() {
        let mut data = [1.0f32, 2.0, 3.0, 4.0];
        MathUtils::<f32>::simd_max(&mut data, 2.5);
        assert_eq!(data, [2.5f32, 2.5, 3.0, 4.0]);
    }

    #[test]
    fn test_simd_copy() {
        let src = [1.0f32, 2.0, 3.0, 4.0];
        let mut dst = [0.0f32; 4];
        MathUtils::<f32>::simd_copy(&src, &mut dst);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_simd_set() {
        let mut data = [0.0f32; 4];
        MathUtils::<f32>::simd_set(&mut data, 1.0);
        assert_eq!(data, [1.0f32; 4]);
    }

    #[test]
    fn test_simd_any() {
        let data = [1.0f32, 2.0, 3.0, 4.0];
        assert!(MathUtils::<f32>::simd_any(&data, 2.0));
        assert!(!MathUtils::<f32>::simd_any(&data, 5.0));
    }

    #[test]
    fn test_simd_not_any() {
        let data = [1.0f32, 2.0, 3.0, 4.0];
        assert!(MathUtils::<f32>::simd_not_any(&data, 5.0));
        assert!(!MathUtils::<f32>::simd_not_any(&data, 2.0));
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vector3<T: Copy> {
    pub x: T,
    pub y: T,
    pub z: T,
}

impl<T: Copy> Vector3<T> {
    pub fn new(x: T, y: T, z: T) -> Self {
        Vector3 { x, y, z }
    }

    pub fn zero() -> Self
    where
        T: Default,
    {
        Vector3 {
            x: T::default(),
            y: T::default(),
            z: T::default(),
        }
    }

    pub fn one() -> Self
    where
        T: From<u8>,
    {
        Vector3 {
            x: T::from(1),
            y: T::from(1),
            z: T::from(1),
        }
    }

    pub fn dot(self, other: Self) -> T
    where
        T: std::ops::Mul<Output = T> + std::ops::Add<Output = T>,
    {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self
    where
        T: std::ops::Mul<Output = T> + std::ops::Sub<Output = T>,
    {
        Vector3 {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
}

impl<T: Copy> std::ops::Add for Vector3<T>
where
    T: std::ops::Add<Output = T>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Vector3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl<T: Copy> std::ops::Sub for Vector3<T>
where
    T: std::ops::Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Vector3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl<T: Copy> std::ops::Mul for Vector3<T>
where
    T: std::ops::Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Vector3 {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }
}

impl<T: Copy> std::ops::Div for Vector3<T>
where
    T: std::ops::Div<Output = T>,
{
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Vector3 {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
        }
    }
}