#![allow(unsafe_op_in_unsafe_fn)]
use crossbeam_channel;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use windows::{
    core::GUID,
    Win32::Media::Audio::*,
    Win32::System::Com::*,
    Win32::System::Performance::QueryPerformanceCounter,
};

const FORMAT_DETECT_TIMEOUT_SECS: u64 = 3;
const AUDIO_INIT_TIMEOUT_SECS: u64 = 2;
const AUDIO_CHANNEL_CAPACITY: usize = 1024;
const WASAPI_BUF_DURATION_TICKS: i64 = 10_000_000;
const POLL_SLEEP_MS: u64 = 1;
const NO_PACKET_SLEEP_MS: u64 = 1;

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

const KSDATAFORMAT_SUBTYPE_PCM: GUID = GUID::from_values(
    0x00000001, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
);
const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID = GUID::from_values(
    0x00000003, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71],
);

#[derive(Debug, Clone, Copy)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    #[allow(dead_code)]
    pub bits_per_sample: u16,
}

#[derive(Debug)]
struct ParsedFormat {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    is_float: bool,
}

enum AudioInitResult {
    Success,
    Failed(String),
}

pub struct AudioCapture {
    stop_flag: Arc<AtomicBool>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

fn com_init_mta() -> Result<(), String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|e| format!("CoInitializeEx failed: {:?}", e))
}

unsafe fn get_default_render_client() -> Result<(IMMDeviceEnumerator, IMMDevice, IAudioClient), String> {
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
    }.map_err(|e| format!("Failed to create IMMDeviceEnumerator: {:?}", e))?;

    let device = unsafe {
        enumerator.GetDefaultAudioEndpoint(eRender, eConsole)
    }.map_err(|e| format!("Failed to get default audio endpoint: {:?}", e))?;

    let audio_client: IAudioClient = unsafe {
        device.Activate(CLSCTX_ALL, None)
    }.map_err(|e| format!("Failed to activate IAudioClient: {:?}", e))?;

    Ok((enumerator, device, audio_client))
}

unsafe fn get_mix_format(audio_client: &IAudioClient) -> Result<*mut WAVEFORMATEX, String> {
    let mix_format = unsafe {
        audio_client.GetMixFormat()
    }.map_err(|e| format!("GetMixFormat failed: {:?}", e))?;
    if mix_format.is_null() {
        return Err("GetMixFormat returned null".to_string());
    }
    Ok(mix_format)
}

unsafe fn parse_wave_format(mix_format: *const WAVEFORMATEX) -> Result<ParsedFormat, String> {
    let wave_format = *mix_format;
    let wtag = wave_format.wFormatTag;
    if wtag == WAVE_FORMAT_EXTENSIBLE {
        let ext = mix_format as *const WAVEFORMATEXTENSIBLE;
        let subfmt = (*ext).SubFormat;
        let is_f32 = subfmt == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
        let is_pcm = subfmt == KSDATAFORMAT_SUBTYPE_PCM;
        if !is_f32 && !is_pcm {
            return Err("Unsupported mix format subformat (neither PCM nor FLOAT)".to_string());
        }
        Ok(ParsedFormat {
            sample_rate: (*ext).Format.nSamplesPerSec,
            channels: (*ext).Format.nChannels,
            bits_per_sample: (*ext).Format.wBitsPerSample,
            is_float: is_f32,
        })
    } else {
        let is_f32 = wtag == WAVE_FORMAT_IEEE_FLOAT;
        let is_pcm = wtag == WAVE_FORMAT_PCM;
        if !is_f32 && !is_pcm {
            return Err(format!("Unsupported wave format tag: {}", wtag));
        }
        Ok(ParsedFormat {
            sample_rate: wave_format.nSamplesPerSec,
            channels: wave_format.nChannels,
            bits_per_sample: wave_format.wBitsPerSample,
            is_float: is_f32,
        })
    }
}

impl AudioCapture {
    pub fn init_format() -> Result<AudioFormat, String> {
        let (tx, rx) = crossbeam_channel::bounded::<Result<AudioFormat, String>>(1);

        std::thread::spawn(move || {
            unsafe {
                let _ = com_init_mta();
                let result = (|| -> Result<AudioFormat, String> {
                    let (_, _, audio_client) = get_default_render_client()?;
                    let mix_format = get_mix_format(&audio_client)?;
                    let parsed = parse_wave_format(mix_format);
                    CoTaskMemFree(Some(mix_format as *mut _));
                    let parsed = parsed?;
                    Ok(AudioFormat {
                        sample_rate: parsed.sample_rate,
                        channels: parsed.channels,
                        bits_per_sample: 16,
                    })
                })();
                CoUninitialize();
                let _ = tx.send(result);
            }
        });

        match rx.recv_timeout(std::time::Duration::from_secs(FORMAT_DETECT_TIMEOUT_SECS)) {
            Ok(Ok(fmt)) => Ok(fmt),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Audio format detection timed out".to_string()),
        }
    }

    /// Starts loopback audio capture. Each packet carries the WASAPI QPC
    /// timestamp (100ns-class counter) of the first sample in the buffer, which
    /// the caller can use to discard audio captured before the video timeline.
    pub fn start() -> Result<(Self, crossbeam_channel::Receiver<(Vec<u8>, i64)>), String> {
        let (audio_tx, audio_rx) =
            crossbeam_channel::bounded::<(Vec<u8>, i64)>(AUDIO_CHANNEL_CAPACITY);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let (init_tx, init_rx) = crossbeam_channel::bounded::<AudioInitResult>(1);

        let handle = std::thread::spawn(move || {
            let result = unsafe {
                if let Err(e) = com_init_mta() {
                    let _ = init_tx.send(AudioInitResult::Failed(e));
                    return;
                }
                let r = run_capture_loop(&audio_tx, &stop_flag_clone, &init_tx);
                CoUninitialize();
                r
            };
            if let Err(e) = result {
                crate::log_line!("[AUDIO] Capture loop error: {}", e);
                let _ = init_tx.try_send(AudioInitResult::Failed(e));
            }
        });

        match init_rx.recv_timeout(std::time::Duration::from_secs(AUDIO_INIT_TIMEOUT_SECS)) {
            Ok(AudioInitResult::Failed(e)) => {
                stop_flag.store(true, Ordering::SeqCst);
                let _ = handle.join();
                Err(e)
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                stop_flag.store(true, Ordering::SeqCst);
                let _ = handle.join();
                Err("Audio capture initialization timed out".to_string())
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err("Audio capture thread exited unexpectedly during init".to_string())
            }
            Ok(AudioInitResult::Success) => {
                Ok((Self { stop_flag, thread_handle: Some(handle) }, audio_rx))
            }
        }
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

fn f32_to_pcm16(input: &[u8]) -> Vec<u8> {
    if input.len() % 4 != 0 {
        return Vec::new();
    }
    let samples = unsafe { std::slice::from_raw_parts(input.as_ptr() as *const f32, input.len() / 4) };
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let sample = if s.is_nan() { 0.0 } else { s.clamp(-1.0, 1.0) };
        let pcm = (sample * 32767.0) as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }
    out
}

fn pcm24_to_pcm16(input: &[u8]) -> Vec<u8> {
    if input.len() % 3 != 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(input.len() / 3 * 2);
    for chunk in input.chunks_exact(3) {
        // Signed little-endian 24-bit sample.
        let sample = (chunk[0] as i32) | ((chunk[1] as i32) << 8) | ((chunk[2] as i32) << 16);
        let sample = sample << 8 >> 8; // sign-extend 24 -> 32 bits
        let pcm = (sample >> 8) as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }
    out
}

fn pcm32_to_pcm16(input: &[u8]) -> Vec<u8> {
    if input.len() % 4 != 0 {
        return Vec::new();
    }
    let samples = unsafe { std::slice::from_raw_parts(input.as_ptr() as *const i32, input.len() / 4) };
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let pcm = (s >> 16) as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
    }
    out
}

fn convert_to_pcm16(raw: &[u8], is_float: bool, bits_per_sample: u16) -> Vec<u8> {
    match (is_float, bits_per_sample) {
        (true, 32) => f32_to_pcm16(raw),
        (false, 16) => raw.to_vec(),
        (false, 24) => pcm24_to_pcm16(raw),
        (false, 32) => pcm32_to_pcm16(raw),
        // Unknown layouts: pass through and let the encoder's alignment checks
        // reject them instead of producing garbage by misinterpreting bytes.
        _ => raw.to_vec(),
    }
}

unsafe fn run_capture_loop(
    tx: &crossbeam_channel::Sender<(Vec<u8>, i64)>,
    stop_flag: &Arc<AtomicBool>,
    init_tx: &crossbeam_channel::Sender<AudioInitResult>,
) -> Result<(), String> {
    let (_, _, audio_client) = get_default_render_client()?;
    let mix_format = get_mix_format(&audio_client)?;

    let parsed = parse_wave_format(mix_format);
    if let Err(e) = &parsed {
        CoTaskMemFree(Some(mix_format as *mut _));
        return Err(e.clone());
    }
    let fmt = parsed.unwrap();

    crate::log_line!(
        "[AUDIO] WASAPI format: {}Hz, {}ch, {}bit, float={}",
        fmt.sample_rate, fmt.channels, fmt.bits_per_sample, fmt.is_float
    );

    let flags = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_NOPERSIST;
    let init_result = audio_client.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        flags,
        WASAPI_BUF_DURATION_TICKS,
        0,
        mix_format,
        None,
    );
    CoTaskMemFree(Some(mix_format as *mut _));
    init_result.map_err(|e| format!("IAudioClient::Initialize failed: {:?}", e))?;

    let capture_client: IAudioCaptureClient = audio_client
        .GetService()
        .map_err(|e| format!("GetService(IAudioCaptureClient) failed: {:?}", e))?;

    audio_client.Start().map_err(|e| format!("IAudioClient::Start failed: {:?}", e))?;
    let _ = init_tx.send(AudioInitResult::Success);

    let block_align = fmt.channels as u32 * fmt.bits_per_sample as u32 / 8;
    let pcm16_block_align = fmt.channels as usize * 2;
    let silent_flag = AUDCLNT_BUFFERFLAGS_SILENT.0 as u32;
    let mut last_packet_time = Instant::now();

    while !stop_flag.load(Ordering::SeqCst) {
        let packet_size = match capture_client.GetNextPacketSize() {
            Ok(size) => size,
            Err(_) => {
                maybe_push_idle_silence(tx, &fmt, &mut last_packet_time);
                std::thread::sleep(std::time::Duration::from_millis(NO_PACKET_SLEEP_MS));
                continue;
            }
        };

        if packet_size == 0 {
            maybe_push_idle_silence(tx, &fmt, &mut last_packet_time);
            std::thread::sleep(std::time::Duration::from_millis(POLL_SLEEP_MS));
            continue;
        }
        last_packet_time = Instant::now();

        let mut frames_remaining = packet_size;
        while frames_remaining > 0 {
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;
            let mut device_position = 0u64;
            let mut qpc_position = 0u64;

            if capture_client
                .GetBuffer(
                    &mut data,
                    &mut num_frames,
                    &mut flags,
                    Some(&mut device_position),
                    Some(&mut qpc_position),
                )
                .is_err()
            {
                break;
            }

            if num_frames > 0 && !data.is_null() {
                let byte_count = num_frames as usize * block_align as usize;
                let raw_slice = std::slice::from_raw_parts(data, byte_count);
                let qpc = qpc_position as i64;

                if (flags & silent_flag) == 0 {
                    let pcm_data = convert_to_pcm16(raw_slice, fmt.is_float, fmt.bits_per_sample);
                    if !pcm_data.is_empty() {
                        let _ = tx.try_send((pcm_data, qpc));
                    }
                } else {
                    // WASAPI marks idle/quiet buffers as SILENT and provides no
                    // samples. Push explicit silence so the audio timeline keeps
                    // advancing during silent scenes; otherwise the first real
                    // sound would be timestamped near the start of the video,
                    // making the audio track play ahead of the picture.
                    let _ = tx.try_send((
                        vec![0u8; num_frames as usize * pcm16_block_align],
                        qpc,
                    ));
                }
            }

            let _ = capture_client.ReleaseBuffer(num_frames);
            frames_remaining = frames_remaining.saturating_sub(num_frames);
        }
    }

    let _ = audio_client.Stop();
    Ok(())
}

/// When WASAPI loopback delivers no packets (no active render stream), the
/// audio timeline would otherwise stall. Push 10ms silence packets at a real
/// time cadence so the encoder keeps a continuous clock. QPC is read directly
/// so the caller can still discard packets captured before the video start.
fn maybe_push_idle_silence(
    tx: &crossbeam_channel::Sender<(Vec<u8>, i64)>,
    fmt: &ParsedFormat,
    last_packet_time: &mut Instant,
) {
    if last_packet_time.elapsed() < std::time::Duration::from_millis(10) {
        return;
    }
    let mut qpc = 0i64;
    let _ = unsafe { QueryPerformanceCounter(&mut qpc) };
    let frames = (fmt.sample_rate / 100).max(1) as usize; // ~10ms
    let _ = tx.try_send((vec![0u8; frames * fmt.channels as usize * 2], qpc));
    *last_packet_time = Instant::now();
}
