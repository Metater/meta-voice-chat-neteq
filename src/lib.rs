use neteq::statistics::{LifetimeStatistics, NetworkStatistics, OperationCounters};
use neteq::{AudioPacket, NetEq, NetEqConfig, NetEqStats, RtpHeader};
use std::mem::{align_of, size_of};

/// C-compatible copy of NetEQ's rolling decode-operation counters.
#[repr(C)]
pub struct NetEqOperationCounters {
    pub normal_per_sec: f32,
    pub expand_per_sec: f32,
    pub accelerate_per_sec: f32,
    pub fast_accelerate_per_sec: f32,
    pub preemptive_expand_per_sec: f32,
    pub merge_per_sec: f32,
    pub comfort_noise_per_sec: f32,
    pub dtmf_per_sec: f32,
    pub undefined_per_sec: f32,
}

/// C-compatible copy of NetEQ's network statistics.
#[repr(C)]
pub struct NetEqNetworkStatistics {
    pub current_buffer_size_ms: u16,
    pub preferred_buffer_size_ms: u16,
    pub jitter_peaks_found: u16,
    pub expand_rate: u16,
    pub speech_expand_rate: u16,
    pub preemptive_rate: u16,
    pub accelerate_rate: u16,
    pub mean_waiting_time_ms: i32,
    pub median_waiting_time_ms: i32,
    pub min_waiting_time_ms: i32,
    pub max_waiting_time_ms: i32,
    pub reordered_packets: u32,
    pub total_packets_received: u32,
    pub reorder_rate_permyriad: u16,
    pub max_reorder_distance: u16,
    pub operation_counters: NetEqOperationCounters,
}

/// C-compatible copy of NetEQ's lifetime statistics.
#[repr(C)]
pub struct NetEqLifetimeStatistics {
    pub total_samples_received: u64,
    pub concealed_samples: u64,
    pub concealment_events: u64,
    pub jitter_buffer_delay_ms: u64,
    pub jitter_buffer_emitted_count: u64,
    pub jitter_buffer_target_delay_ms: u64,
    pub inserted_samples_for_deceleration: u64,
    pub removed_samples_for_acceleration: u64,
    pub silent_concealed_samples: u64,
    pub relative_packet_arrival_delay_ms: u64,
    pub jitter_buffer_packets_received: u64,
    pub buffer_flushes: u64,
    pub late_packets_discarded: u64,
}

/// C-compatible copy of the statistics returned by [`NetEq::get_statistics`].
///
/// `packets_awaiting_decode` is represented as `u64` so its layout is stable
/// across 32-bit and 64-bit callers.
#[repr(C)]
pub struct NetEqStatistics {
    pub network: NetEqNetworkStatistics,
    pub lifetime: NetEqLifetimeStatistics,
    pub current_buffer_size_ms: u32,
    pub target_delay_ms: u32,
    pub packets_awaiting_decode: u64,
    pub packets_per_sec: u32,
}

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

/// Copy the current NetEQ statistics into `statistics`.
///
/// Returns `1` on success. Returns `0` when either pointer is null or the
/// output pointer is not correctly aligned; in that case no data is written.
#[unsafe(no_mangle)]
pub extern "C" fn get_statistics(ptr: *mut NetEq, statistics: *mut NetEqStatistics) -> i32 {
    if ptr.is_null() || statistics.is_null() || !is_aligned::<NetEqStatistics>(statistics) {
        return 0;
    }

    let neteq = unsafe { &*ptr };
    let result = NetEqStatistics::from(neteq.get_statistics());
    unsafe {
        statistics.write(result);
    }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn is_empty(ptr: *mut NetEq) -> i32 {
    if ptr.is_null() {
        return 1;
    }

    i32::from(unsafe { (&*ptr).is_empty() })
}

#[unsafe(no_mangle)]
pub extern "C" fn target_delay_ms(ptr: *mut NetEq) -> i32 {
    if ptr.is_null() {
        return 0;
    }

    saturating_i32_from_u32(unsafe { (&*ptr).target_delay_ms() })
}

#[unsafe(no_mangle)]
pub extern "C" fn set_minimum_delay(ptr: *mut NetEq, delay_ms: i32) -> i32 {
    if ptr.is_null() {
        return 0;
    }

    saturating_i32_from_u32(unsafe { (&mut *ptr).set_minimum_delay(to_u32(delay_ms)) })
}

#[unsafe(no_mangle)]
pub extern "C" fn set_maximum_delay(ptr: *mut NetEq, delay_ms: i32) -> i32 {
    if ptr.is_null() {
        return 0;
    }

    saturating_i32_from_u32(unsafe { (&mut *ptr).set_maximum_delay(to_u32(delay_ms)) })
}

#[unsafe(no_mangle)]
pub extern "C" fn flush(ptr: *mut NetEq) {
    if !ptr.is_null() {
        unsafe { (&mut *ptr).flush() };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn current_buffer_size_samples(ptr: *mut NetEq) -> i32 {
    if ptr.is_null() {
        return 0;
    }

    saturating_i32_from_usize(unsafe { (&*ptr).current_buffer_size_samples() })
}

impl From<NetEqStats> for NetEqStatistics {
    fn from(stats: NetEqStats) -> Self {
        Self {
            network: stats.network.into(),
            lifetime: stats.lifetime.into(),
            current_buffer_size_ms: stats.current_buffer_size_ms,
            target_delay_ms: stats.target_delay_ms,
            packets_awaiting_decode: stats.packets_awaiting_decode.min(u64::MAX as usize) as u64,
            packets_per_sec: stats.packets_per_sec,
        }
    }
}

impl From<NetworkStatistics> for NetEqNetworkStatistics {
    fn from(stats: NetworkStatistics) -> Self {
        Self {
            current_buffer_size_ms: stats.current_buffer_size_ms,
            preferred_buffer_size_ms: stats.preferred_buffer_size_ms,
            jitter_peaks_found: stats.jitter_peaks_found,
            expand_rate: stats.expand_rate,
            speech_expand_rate: stats.speech_expand_rate,
            preemptive_rate: stats.preemptive_rate,
            accelerate_rate: stats.accelerate_rate,
            mean_waiting_time_ms: stats.mean_waiting_time_ms,
            median_waiting_time_ms: stats.median_waiting_time_ms,
            min_waiting_time_ms: stats.min_waiting_time_ms,
            max_waiting_time_ms: stats.max_waiting_time_ms,
            reordered_packets: stats.reordered_packets,
            total_packets_received: stats.total_packets_received,
            reorder_rate_permyriad: stats.reorder_rate_permyriad,
            max_reorder_distance: stats.max_reorder_distance,
            operation_counters: stats.operation_counters.into(),
        }
    }
}

impl From<OperationCounters> for NetEqOperationCounters {
    fn from(counters: OperationCounters) -> Self {
        Self {
            normal_per_sec: counters.normal_per_sec,
            expand_per_sec: counters.expand_per_sec,
            accelerate_per_sec: counters.accelerate_per_sec,
            fast_accelerate_per_sec: counters.fast_accelerate_per_sec,
            preemptive_expand_per_sec: counters.preemptive_expand_per_sec,
            merge_per_sec: counters.merge_per_sec,
            comfort_noise_per_sec: counters.comfort_noise_per_sec,
            dtmf_per_sec: counters.dtmf_per_sec,
            undefined_per_sec: counters.undefined_per_sec,
        }
    }
}

impl From<LifetimeStatistics> for NetEqLifetimeStatistics {
    fn from(stats: LifetimeStatistics) -> Self {
        Self {
            total_samples_received: stats.total_samples_received,
            concealed_samples: stats.concealed_samples,
            concealment_events: stats.concealment_events,
            jitter_buffer_delay_ms: stats.jitter_buffer_delay_ms,
            jitter_buffer_emitted_count: stats.jitter_buffer_emitted_count,
            jitter_buffer_target_delay_ms: stats.jitter_buffer_target_delay_ms,
            inserted_samples_for_deceleration: stats.inserted_samples_for_deceleration,
            removed_samples_for_acceleration: stats.removed_samples_for_acceleration,
            silent_concealed_samples: stats.silent_concealed_samples,
            relative_packet_arrival_delay_ms: stats.relative_packet_arrival_delay_ms,
            jitter_buffer_packets_received: stats.jitter_buffer_packets_received,
            buffer_flushes: stats.buffer_flushes,
            late_packets_discarded: stats.late_packets_discarded,
        }
    }
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

fn saturating_i32_from_usize(value: usize) -> i32 {
    value.min(i32::MAX as usize) as i32
}

fn is_aligned<T>(ptr: *const T) -> bool {
    (ptr as usize) % align_of::<T>() == 0
}

fn is_aligned_for_f32(ptr: *const f32) -> bool {
    is_aligned(ptr)
}
