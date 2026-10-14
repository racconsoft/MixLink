//! The real-time mixer node. pipewire-rs 0.10 has no safe pw_filter wrapper, so this
//! uses the pipewire-sys FFI: 16 stereo input slots, monitor and stream stereo outputs.
use std::ffi::{CString, c_void};
use std::ptr;
use std::sync::Arc;

use mixlink_core::model::MAX_CHANNELS;
use pipewire as pw;
use pipewire_sys as pws;
use pw::properties::properties;

use super::graph::{MONITOR_OUT, STREAM_OUT, mixer_input_port};
use crate::mixer::dsp::{self, Block, DspState, Peaks};
use crate::mixer::shared::SharedParams;

const MAX_BLOCK: usize = 8192;
const DEFAULT_RATE: u32 = 48_000;

/// Owned by the RT callback after `pw_filter_connect`; only `shared` is touched elsewhere.
struct RtData {
    shared: Arc<SharedParams>,
    inputs: [[*mut c_void; 2]; MAX_CHANNELS],
    /// monitor L, monitor R, stream L, stream R
    outputs: [*mut c_void; 4],
    state: DspState,
    scratch: [Box<[f32]>; 4],
    peaks: Peaks,
}

pub struct MixerNode {
    filter: *mut pws::pw_filter,
    rt: *mut RtData,
    _hook: Box<libspa_sys::spa_hook>,
    _events: Box<pws::pw_filter_events>,
    /// Keeps the core (and its context) alive until `Drop` has destroyed the filter;
    /// fields drop after `Drop::drop`, so this is released last.
    _core: pw::core::CoreRc,
}

impl MixerNode {
    pub fn new(
        core: &pw::core::CoreRc,
        name: &str,
        shared: Arc<SharedParams>,
    ) -> Result<Self, String> {
        let rt = Box::into_raw(Box::new(RtData {
            shared,
            inputs: [[ptr::null_mut(); 2]; MAX_CHANNELS],
            outputs: [ptr::null_mut(); 4],
            state: DspState::default(),
            scratch: std::array::from_fn(|_| vec![0.0; MAX_BLOCK].into_boxed_slice()),
            peaks: Peaks::default(),
        }));
        let events = Box::new(pws::pw_filter_events {
            version: pws::PW_VERSION_FILTER_EVENTS,
            destroy: None,
            state_changed: None,
            io_changed: None,
            param_changed: None,
            add_buffer: None,
            remove_buffer: None,
            process: Some(on_process),
            drained: None,
            command: None,
        });
        // SAFETY: spa_hook is a plain C struct that is valid when zeroed.
        let mut hook: Box<libspa_sys::spa_hook> = Box::new(unsafe { std::mem::zeroed() });
        let props = properties! {
            "media.type" => "Audio",
            "media.category" => "Filter",
            "media.role" => "DSP",
            "node.name" => name,
            "node.description" => "MixLink Mixer",
            "node.always-process" => "true",
        };
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        // SAFETY: all pointers passed to PipeWire stay alive as long as the filter:
        // `rt`, `hook`, `events` and a clone of `core` are owned by the returned
        // MixerNode, which destroys the filter before freeing them.
        unsafe {
            let filter = pws::pw_filter_new(core.as_raw_ptr(), cname.as_ptr(), props.into_raw());
            if filter.is_null() {
                drop(Box::from_raw(rt));
                return Err("pw_filter_new failed".into());
            }
            pws::pw_filter_add_listener(
                filter,
                &mut *hook as *mut libspa_sys::spa_hook as *mut _,
                &*events,
                rt.cast(),
            );
            for slot in 0..MAX_CHANNELS {
                for (c, channel) in ["FL", "FR"].iter().enumerate() {
                    (*rt).inputs[slot][c] = add_port(
                        filter,
                        pws::PW_DIRECTION_INPUT,
                        &mixer_input_port(slot, channel),
                        channel,
                    );
                }
            }
            for (i, port) in MONITOR_OUT.iter().chain(STREAM_OUT.iter()).enumerate() {
                let channel = if i % 2 == 0 { "FL" } else { "FR" };
                (*rt).outputs[i] = add_port(filter, pws::PW_DIRECTION_OUTPUT, port, channel);
            }
            let missing = (*rt)
                .inputs
                .iter()
                .flatten()
                .chain((*rt).outputs.iter())
                .any(|p| p.is_null());
            let connected = !missing
                && pws::pw_filter_connect(
                    filter,
                    pws::pw_filter_flags_PW_FILTER_FLAG_RT_PROCESS,
                    ptr::null_mut(),
                    0,
                ) >= 0;
            if !connected {
                pws::pw_filter_destroy(filter);
                drop(Box::from_raw(rt));
                return Err("creating the mixer ports or connecting the filter failed".into());
            }
            Ok(Self {
                filter,
                rt,
                _hook: hook,
                _events: events,
                _core: core.clone(),
            })
        }
    }
}

impl Drop for MixerNode {
    fn drop(&mut self) {
        // SAFETY: pw_filter_destroy disconnects synchronously from the data loop, so the
        // process callback can no longer run when RtData is freed afterwards.
        unsafe {
            pws::pw_filter_destroy(self.filter);
            drop(Box::from_raw(self.rt));
        }
    }
}

/// SAFETY: caller passes a valid filter.
unsafe fn add_port(
    filter: *mut pws::pw_filter,
    direction: u32,
    name: &str,
    channel: &str,
) -> *mut c_void {
    let props = properties! {
        "format.dsp" => "32 bit float mono audio",
        "port.name" => name,
        "audio.channel" => channel,
    };
    unsafe {
        pws::pw_filter_add_port(
            filter,
            direction,
            pws::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            0,
            props.into_raw(),
            ptr::null_mut(),
            0,
        )
    }
}

/// Real-time callback: no allocation, no locks, no logging.
unsafe extern "C" fn on_process(data: *mut c_void, position: *mut libspa_sys::spa_io_position) {
    if data.is_null() || position.is_null() {
        return;
    }
    // SAFETY: `data` is the RtData registered in MixerNode::new; only this callback
    // touches its state, scratch and peaks after the filter is connected.
    let rt = unsafe { &mut *(data as *mut RtData) };
    let (duration, rate) = unsafe {
        (
            (*position).clock.duration as usize,
            (*position).clock.rate.denom,
        )
    };
    let n = duration.min(MAX_BLOCK);
    let rate = if rate == 0 { DEFAULT_RATE } else { rate };
    let RtData {
        shared,
        inputs,
        outputs,
        state,
        scratch,
        peaks,
    } = rt;

    let targets = shared.load_targets();
    let mut ins: [[Option<&[f32]>; 2]; MAX_CHANNELS] = [[None; 2]; MAX_CHANNELS];
    for (slot, ports) in inputs.iter().enumerate() {
        for c in 0..2 {
            // SAFETY: port data pointers come from pw_filter_add_port and live as long as the filter.
            let buf = unsafe { pws::pw_filter_get_dsp_buffer(ports[c], n as u32) } as *const f32;
            if !buf.is_null() {
                ins[slot][c] = Some(unsafe { std::slice::from_raw_parts(buf, n) });
            }
        }
    }

    *peaks = Peaks::default();
    {
        let [m_l, m_r, s_l, s_r] = scratch;
        dsp::process(
            state,
            &targets,
            Block {
                inputs: ins,
                monitor: [&mut m_l[..n], &mut m_r[..n]],
                stream: [&mut s_l[..n], &mut s_r[..n]],
            },
            dsp::ramp_step(rate),
            peaks,
        );
    }
    for (port, buf) in outputs.iter().zip(scratch.iter()) {
        let out = unsafe { pws::pw_filter_get_dsp_buffer(*port, n as u32) } as *mut f32;
        if !out.is_null() {
            // SAFETY: PipeWire hands out at least `n` samples for this cycle.
            unsafe { ptr::copy_nonoverlapping(buf.as_ptr(), out, n) };
        }
    }
    shared.publish_peaks(peaks);
}
