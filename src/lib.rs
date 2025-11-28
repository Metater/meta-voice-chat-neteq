use neteq::{AudioPacket, NetEq, NetEqConfig, RtpHeader};

#[unsafe(no_mangle)]
pub extern "C" fn create_neteq(
    sample_rate: u32,
    channels: u8,
    max_packets_in_buffer: i32,
    max_delay_ms: u32,
    min_delay_ms: u32,
    additional_delay_ms: u32,
) -> *mut NetEq {
    let config = NetEqConfig {
        sample_rate,
        channels,
        max_packets_in_buffer: max_packets_in_buffer as usize,
        max_delay_ms,
        min_delay_ms,
        additional_delay_ms,
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
    sample_rate: u32,
    channels: u8,
    duration_ms: u32,
) {
    if ptr.is_null() {
        return;
    }

    let neteq = unsafe { &mut *ptr };

    let header = RtpHeader::new(sequence_number, timestamp, 12345, 96, false);

    let mut payload = Vec::new();

    // Generate sine wave audio data
    // let frequency1 = 659.26; // E5 note
    // let frequency2 = 523.25; // C5 note

    // for i in 0..samples {
    //     let t = (timestamp as f32 + i as f32) / sample_rate as f32;
    //     let sample = (2.0 * std::f32::consts::PI * frequency1 * t).sin() * 0.1;
    //     let sample2 = (2.0 * std::f32::consts::PI * frequency2 * t).sin() * 0.1;
    //     let combined_sample = sample + sample2;
    //     payload.extend_from_slice(&combined_sample.to_le_bytes());
    // }

    let samples_slice = unsafe { std::slice::from_raw_parts(samples, samples_len as usize) };
    for &sample in samples_slice {
        payload.extend_from_slice(&sample.to_le_bytes());
    }

    let packet = AudioPacket::new(header, payload, sample_rate, channels, duration_ms);

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
pub extern "C" fn current_buffer_size_ms(ptr: *mut NetEq) -> u32 {
    if ptr.is_null() {
        return 0;
    }

    let neteq = unsafe { &mut *ptr };
    neteq.current_buffer_size_ms()
}
