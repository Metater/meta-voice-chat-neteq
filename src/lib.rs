use neteq::{AudioPacket, NetEq, NetEqConfig, RtpHeader};
use std::mem::{align_of, size_of};

#[unsafe(no_mangle)]
pub extern "C" fn create_neteq(
    sample_rate: i32,
    channels: i32,
    max_packets_in_buffer: i32,
    max_delay_ms: i32,
    min_delay_ms: i32,
    additional_delay_ms: i32,
) -> *mut NetEq {
    let config = NetEqConfig {
        sample_rate: to_u32(sample_rate),
        channels: to_u8(channels),
        max_packets_in_buffer: to_usize(max_packets_in_buffer),
        max_delay_ms: to_u32(max_delay_ms),
        min_delay_ms: to_u32(min_delay_ms),
        additional_delay_ms: to_u32(additional_delay_ms),
        ..Default::default()
    };

    match NetEq::new(config) {
        Ok(neteq) => Box::into_raw(Box::new(neteq)),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn free_neteq(ptr: *mut NetEq) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn insert_packet(
    ptr: *mut NetEq,
    sequence_number: u16,
    timestamp: u32,
    samples: *mut f32,
    samples_len: i32,
    sample_rate: i32,
    channels: i32,
    duration_ms: i32,
) {
    if ptr.is_null() {
        return;
    }

    if samples.is_null() || samples_len <= 0 {
        return;
    }

    if !is_aligned_for_f32(samples) {
        return;
    }

    let neteq = unsafe { &mut *ptr };

    let header = RtpHeader::new(sequence_number, timestamp, 12345, 96, false);

    let samples_len = samples_len as usize;
    let mut payload = Vec::with_capacity(samples_len.saturating_mul(size_of::<f32>()));
    let samples_slice = unsafe { std::slice::from_raw_parts(samples, samples_len) };
    for &sample in samples_slice {
        payload.extend_from_slice(&sample.to_le_bytes());
    }

    let packet = AudioPacket::new(
        header,
        payload,
        to_u32(sample_rate),
        to_u8(channels),
        to_u32(duration_ms),
    );

    let _ = neteq.insert_packet(packet);
}

#[unsafe(no_mangle)]
pub extern "C" fn get_audio(ptr: *mut NetEq, samples: *mut f32, samples_len: i32) -> i32 {
    if ptr.is_null() {
        return 0;
    }

    if samples.is_null() || samples_len <= 0 {
        return 0;
    }

    if !is_aligned_for_f32(samples) {
        return 0;
    }

    let neteq = unsafe { &mut *ptr };

    let frame = match neteq.get_audio() {
        Ok(f) => f,
        Err(_) => return 0,
    };

    let samples_to_copy = std::cmp::min(frame.samples.len() as i32, samples_len);
    unsafe {
        std::ptr::copy_nonoverlapping(frame.samples.as_ptr(), samples, samples_to_copy as usize);
    }

    samples_to_copy
}

#[unsafe(no_mangle)]
pub extern "C" fn current_buffer_size_ms(ptr: *mut NetEq) -> i32 {
    if ptr.is_null() {
        return 0;
    }

    let neteq = unsafe { &mut *ptr };
    saturating_i32_from_u32(neteq.current_buffer_size_ms())
}

fn to_u8(value: i32) -> u8 {
    value.clamp(0, u8::MAX as i32) as u8
}

fn to_u32(value: i32) -> u32 {
    value.max(0) as u32
}

fn to_usize(value: i32) -> usize {
    value.max(0) as usize
}

fn saturating_i32_from_u32(value: u32) -> i32 {
    value.min(i32::MAX as u32) as i32
}

fn is_aligned_for_f32(ptr: *const f32) -> bool {
    (ptr as usize) % align_of::<f32>() == 0
}
