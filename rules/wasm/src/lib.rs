//! C ABI over the generated Ember Reach rules.
//!
//! The rules themselves live in `rules/ox/reach.ox` (Oxidized) and are
//! transpiled to `rules/ox_gen/reach_gen.rs`. This crate only marshals the
//! rules' own data format (a flat `Vec<i64>` state, a `[i64; 4]` action) across
//! the wasm boundary, so the browser and the Site Worker run the exact same
//! rules code — no second, hand-ported implementation to drift.
//!
//! Every out-parameter is a boxed slice the caller must release with
//! `ox_free(ptr, len)`.

#![allow(clippy::all)]

#[allow(
    dead_code,
    unused_imports,
    unused_variables,
    unused_mut,
    unused_parens,
    unused_assignments,
    non_snake_case,
    non_camel_case_types,
    clippy::all
)]
mod ox {
    include!("../../ox_gen/reach_gen.rs");
}

use ox::DynamicValue as Dv;
use std::slice;

/// Layout constants from `ox/reach.ox` (see `p_base`, `p_stride`, `h_rng`).
const P_BASE: usize = 40;
const P_STRIDE: usize = 20;
const HAND_OFFSET: usize = 11;
const RNG_INDEX: usize = 5;

fn to_dv(v: &[i64]) -> Dv {
    Dv::list(v.iter().map(|x| Dv::Int(*x)).collect())
}

fn from_dv(d: Dv) -> Vec<i64> {
    match d {
        Dv::List(l) => l.into_vec().into_iter().map(|x| x.as_i64()).collect(),
        _ => Vec::new(),
    }
}

unsafe fn input<'a>(ptr: *const i64, len: usize) -> &'a [i64] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        slice::from_raw_parts(ptr, len)
    }
}

fn output(values: Vec<i64>, out_len: *mut usize) -> *mut i64 {
    let boxed = values.into_boxed_slice();
    let len = boxed.len();
    let ptr = Box::into_raw(boxed) as *mut i64;
    if !out_len.is_null() {
        unsafe { *out_len = len }
    }
    ptr
}

/// Allocate `len` i64s the caller may fill and hand back to `ox_apply`/`ox_free`.
#[no_mangle]
pub extern "C" fn ox_alloc(len: usize) -> *mut i64 {
    let mut values: Vec<i64> = vec![0; len];
    let ptr = values.as_mut_ptr();
    std::mem::forget(values);
    ptr
}

#[no_mangle]
pub extern "C" fn ox_free(ptr: *mut i64, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len))) }
}

#[no_mangle]
pub extern "C" fn ox_state_len() -> usize {
    461
}

#[no_mangle]
pub extern "C" fn ox_new_game(players: i64, seed: i64, setup: i64, out_len: *mut usize) -> *mut i64 {
    let state = ox::new_game(Dv::Int(players), Dv::Int(seed), Dv::Int(setup));
    output(from_dv(state), out_len)
}

/// `[count, a0, a1, a2, a3, a0, ...]`.
#[no_mangle]
pub extern "C" fn ox_legal(state: *const i64, len: usize, out_len: *mut usize) -> *mut i64 {
    let s = unsafe { input(state, len) };
    let mut flat: Vec<i64> = Vec::new();
    if let Dv::List(items) = ox::legal_actions(to_dv(s)) {
        let actions: Vec<Vec<i64>> = items.into_vec().into_iter().map(from_dv).collect();
        flat.push(actions.len() as i64);
        for action in actions {
            flat.extend_from_slice(&action);
        }
    } else {
        flat.push(0);
    }
    output(flat, out_len)
}

#[no_mangle]
pub extern "C" fn ox_is_legal(
    state: *const i64,
    len: usize,
    a0: i64,
    a1: i64,
    a2: i64,
    a3: i64,
) -> i32 {
    let s = unsafe { input(state, len) };
    ox::is_legal(to_dv(s), to_dv(&[a0, a1, a2, a3])).as_bool() as i32
}

/// Empty output means the action was not legal.
#[no_mangle]
pub extern "C" fn ox_apply(
    state: *const i64,
    len: usize,
    a0: i64,
    a1: i64,
    a2: i64,
    a3: i64,
    out_len: *mut usize,
) -> *mut i64 {
    let s = unsafe { input(state, len) };
    let action = [a0, a1, a2, a3];
    if !ox::is_legal(to_dv(s), to_dv(&action)).as_bool() {
        return output(Vec::new(), out_len);
    }
    output(from_dv(ox::apply(to_dv(s), to_dv(&action))), out_len)
}

#[no_mangle]
pub extern "C" fn ox_bot_choose(
    state: *const i64,
    len: usize,
    out_len: *mut usize,
) -> *mut i64 {
    let s = unsafe { input(state, len) };
    output(from_dv(ox::bot_choose(to_dv(s))), out_len)
}

#[no_mangle]
pub extern "C" fn ox_current(state: *const i64, len: usize) -> i64 {
    let s = unsafe { input(state, len) };
    s.get(3).copied().unwrap_or(-1)
}

#[no_mangle]
pub extern "C" fn ox_over(state: *const i64, len: usize) -> i32 {
    let s = unsafe { input(state, len) };
    ox::is_over(to_dv(s)).as_bool() as i32
}

#[no_mangle]
pub extern "C" fn ox_winner(state: *const i64, len: usize) -> i64 {
    let s = unsafe { input(state, len) };
    ox::winner(to_dv(s)).as_i64()
}

#[no_mangle]
pub extern "C" fn ox_setup_count(players: i64) -> i64 {
    ox::setup_count(Dv::Int(players)).as_i64()
}

/// The state as one player may see it: other hands are blanked and the RNG is
/// cleared so the upcoming deck order cannot be predicted from the payload.
#[no_mangle]
pub extern "C" fn ox_redact(
    state: *const i64,
    len: usize,
    viewer: i64,
    out_len: *mut usize,
) -> *mut i64 {
    let s = unsafe { input(state, len) };
    let mut values = s.to_vec();
    if values.len() > RNG_INDEX {
        values[RNG_INDEX] = 0;
    }
    let players = values.first().copied().unwrap_or(0).max(0) as usize;
    for p in 0..players.min(4) {
        if p as i64 == viewer {
            continue;
        }
        let index = P_BASE + P_STRIDE * p + HAND_OFFSET;
        if index < values.len() {
            values[index] = 0;
        }
    }
    output(values, out_len)
}
