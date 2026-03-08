macro_rules! make_slice {
    // Make a slice from an array
    // Where the length is length * ch
    ($arr:expr, $length:expr, $ch:expr) => {{
        let size = ($length as usize) * ($ch as usize);
        if $arr.len() < size {
            panic!("Array is too small for the given length and channels");
        }
        &$arr[..size]
    }};
}

pub(super) use make_slice;

macro_rules! make_slice_mut {
    // Make a mutable slice from an array
    // Where the length is length * ch
    ($arr:expr, $length:expr, $ch:expr) => {{
        let size = ($length as usize) * ($ch as usize);
        if $arr.len() < size {
            panic!("Array is too small for the given length and channels");
        }
        &mut $arr[..size]
    }};
}

pub(super) use make_slice_mut;

macro_rules! array_len_from {
    ($len:expr, $ch:expr) => {
        ($len as usize) * ($ch as usize)
    };
}

pub(super) use array_len_from;

macro_rules! frame_count_from {
    ($len:expr, $ch:expr) => {
        ($len as usize) / ($ch as usize)
    };
}

pub(super) use frame_count_from;

macro_rules! check {
    ($result:expr, $mapper:expr) => {{
        let res = $result;
        if res.is_err() {
            return Err($mapper);
        }
        res.unwrap()
    }};
}

pub(super) use check;

macro_rules! check_ret {
    ($result:expr, $mapper:expr) => {{
        let res = $result;
        if res.is_err() {
            return Err($mapper(res.err().unwrap()));
        }
        res.unwrap()
    }};
}

pub(super) use check_ret;