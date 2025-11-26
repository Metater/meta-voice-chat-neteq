use neteq::{AudioPacket, NetEq, NetEqConfig, RtpHeader};
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Mutex};

static INSTANCES: Lazy<Mutex<HashMap<u64, NetEq>>> = Lazy::new(|| {
    let m = HashMap::new();
    Mutex::new(m)
});

#[unsafe(no_mangle)]
pub extern "C" fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[unsafe(no_mangle)]
pub extern "C" fn instantiate(
    handle: u64,

    sample_rate: u32,
    channels: u8,
    max_packets_in_buffer: i32,
    max_delay_ms: u32,
    min_delay_ms: u32,
    additional_delay_ms: u32,
) {
    let config = NetEqConfig {
        sample_rate,
        channels,
        max_packets_in_buffer: max_packets_in_buffer as usize,
        max_delay_ms,
        min_delay_ms,
        additional_delay_ms,
        ..Default::default()
    };

    let neteq = NetEq::new(config).unwrap();
    INSTANCES.lock().unwrap().insert(handle, neteq);
}

#[unsafe(no_mangle)]
pub extern "C" fn destroy(handle: u64) {
    let mut instances = INSTANCES.lock().unwrap();
    instances.remove(&handle);
}

#[unsafe(no_mangle)]
pub extern "C" fn insert_packet(handle: u64, sequence_number: u16, timestamp: u32) {
    let mut instances = INSTANCES.lock().unwrap();
    let neteq = instances.get_mut(&handle).unwrap();

    let packet = create_audio_packet(sequence_number, timestamp, 480, 48000, 10);
    neteq.insert_packet(packet).unwrap();
}

#[unsafe(no_mangle)]
pub extern "C" fn get_audio(handle: u64, output_buffer: *mut f32, buffer_size: i32) -> i32 {
    let mut instances = INSTANCES.lock().unwrap();
    let neteq = instances.get_mut(&handle).unwrap();

    let frame = neteq.get_audio().unwrap();
    let samples_to_copy = std::cmp::min(frame.samples.len() as i32, buffer_size);
    unsafe {
        std::ptr::copy_nonoverlapping(
            frame.samples.as_ptr(),
            output_buffer,
            samples_to_copy as usize,
        );
    }

    samples_to_copy
}

fn create_audio_packet(
    sequence_number: u16,
    timestamp: u32,
    samples: usize,
    sample_rate: u32,
    duration_ms: u32,
) -> AudioPacket {
    let header = RtpHeader::new(sequence_number, timestamp, 12345, 96, false);

    let mut payload = Vec::new();

    // Generate sine wave audio data
    let frequency1 = 659.26; // E5 note
    let frequency2 = 523.25; // C5 note

    for i in 0..samples {
        let t = (timestamp as f32 + i as f32) / sample_rate as f32;
        let sample = (2.0 * std::f32::consts::PI * frequency1 * t).sin() * 0.1;
        let sample2 = (2.0 * std::f32::consts::PI * frequency2 * t).sin() * 0.1;
        let combined_sample = sample + sample2;
        payload.extend_from_slice(&combined_sample.to_le_bytes());
    }

    AudioPacket::new(header, payload, sample_rate, 1, duration_ms)
}
