use std::ffi::{CStr, CString, c_char, c_int, c_uint};
use std::ptr;

use crate::engine::{Engine, ExecArgs};
use crate::parser;
use crate::registry::{Registry, YAMAMVA_END};
use crate::save;

#[repr(C)]
pub struct FfiElement {
    pub key: *const c_char,
    pub label: *const c_char,
    pub extra_json: *const c_char,
}

#[repr(C)]
pub struct FfiArgs {
    pub node_type: *const c_char,
    pub node_json: *const c_char,
    pub element_count: u32,
    pub elements: *const FfiElement,
    pub result: *const c_char,
}

pub struct FfiState {
    engine: Engine,
    exec_args: ExecArgs,
    // Owned strings kept alive for the duration of the current exec call
    _node_type: Option<CString>,
    _node_json: Option<CString>,
    _elements: Vec<FfiElement>,
    _element_strings: Vec<(CString, CString, CString)>,
}

/// # Safety
/// `yaml_ptr` must be a valid pointer to a UTF-8 string of `yaml_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_load(yaml_ptr: *const c_char, yaml_len: c_uint) -> *mut FfiState {
    if yaml_ptr.is_null() {
        return ptr::null_mut();
    }

    let slice = unsafe { std::slice::from_raw_parts(yaml_ptr as *const u8, yaml_len as usize) };
    let yaml_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let scenario = match parser::parse(yaml_str) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let registry = Registry::new();
    let engine = Engine::new(scenario, registry);

    let state = Box::new(FfiState {
        engine,
        exec_args: ExecArgs::new(),
        _node_type: None,
        _node_json: None,
        _elements: Vec::new(),
        _element_strings: Vec::new(),
    });

    Box::into_raw(state)
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load`.
/// `node_type` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_register(
    h: *mut FfiState,
    node_type: *const c_char,
    command_id: c_int,
    flags: c_int,
) {
    if h.is_null() || node_type.is_null() {
        return;
    }
    let state = unsafe { &mut *h };
    let nt = unsafe { CStr::from_ptr(node_type) };
    if let Ok(s) = nt.to_str() {
        state.engine.registry_mut().register(s, command_id, flags);
    }
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load`.
/// `args` must be a valid pointer to an `FfiArgs` struct.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_exec(h: *mut FfiState, args: *mut FfiArgs) -> c_int {
    if h.is_null() || args.is_null() {
        return YAMAMVA_END;
    }
    let state = unsafe { &mut *h };
    let ffi_args = unsafe { &mut *args };

    // Read result from game side if provided
    if !ffi_args.result.is_null() {
        let result_cstr = unsafe { CStr::from_ptr(ffi_args.result) };
        if let Ok(r) = result_cstr.to_str() {
            state.exec_args.result = Some(r.to_string());
        }
        ffi_args.result = ptr::null();
    }

    let id = state.engine.exec(&mut state.exec_args);

    // Clear old FFI data
    state._node_type = None;
    state._node_json = None;
    state._elements.clear();
    state._element_strings.clear();

    if id == YAMAMVA_END {
        ffi_args.node_type = ptr::null();
        ffi_args.node_json = ptr::null();
        ffi_args.element_count = 0;
        ffi_args.elements = ptr::null();
        return YAMAMVA_END;
    }

    if let Some(ref built) = state.exec_args.built {
        let nt = CString::new(built.node_type.as_str()).unwrap_or_default();
        let nj = CString::new(built.node_json.as_str()).unwrap_or_default();

        state._node_type = Some(nt);
        state._node_json = Some(nj);
        ffi_args.node_type = state._node_type.as_ref().unwrap().as_ptr();
        ffi_args.node_json = state._node_json.as_ref().unwrap().as_ptr();

        let mut element_strings = Vec::new();
        for el in &built.elements {
            let key = CString::new(el.key.as_str()).unwrap_or_default();
            let label = CString::new(el.label.as_deref().unwrap_or("")).unwrap_or_default();
            let extra = CString::new(el.extra_json.as_str()).unwrap_or_default();
            element_strings.push((key, label, extra));
        }
        state._element_strings = element_strings;

        state._elements = state._element_strings.iter().map(|(k, l, e)| {
            FfiElement {
                key: k.as_ptr(),
                label: l.as_ptr(),
                extra_json: e.as_ptr(),
            }
        }).collect();

        ffi_args.element_count = state._elements.len() as u32;
        ffi_args.elements = state._elements.as_ptr();
    }

    id
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load`.
/// `key` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_get_state(h: *const FfiState, key: *const c_char) -> *const c_char {
    if h.is_null() || key.is_null() {
        return ptr::null();
    }
    let state = unsafe { &*h };
    let key_cstr = unsafe { CStr::from_ptr(key) };
    let key_str = match key_cstr.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };

    let val = state.engine.state().get_value(key_str);
    let json_str = match val {
        Some(v) => serde_json::to_string(&v.to_json()).unwrap_or_else(|_| "null".into()),
        None => "null".to_string(),
    };

    match CString::new(json_str) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null(),
    }
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load`.
/// `key` and `value_json` must be valid null-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_set_state(h: *mut FfiState, key: *const c_char, value_json: *const c_char) {
    if h.is_null() || key.is_null() || value_json.is_null() {
        return;
    }
    let state = unsafe { &mut *h };
    let key_str = match unsafe { CStr::from_ptr(key) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    let val_str = match unsafe { CStr::from_ptr(value_json) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };

    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(val_str)
        && let Some(v) = crate::state::Value::from_json(&json_val) {
            state.engine.state_mut().set(key_str, v);
        }
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load`.
/// `section` must be a valid null-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_meta(h: *const FfiState, section: *const c_char) -> *const c_char {
    if h.is_null() || section.is_null() {
        return ptr::null();
    }
    let state = unsafe { &*h };
    let sec_str = match unsafe { CStr::from_ptr(section) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null(),
    };

    let json_str = if sec_str == "state" {
        serde_json::to_string(&state.engine.state().dump()).unwrap_or_else(|_| "{}".into())
    } else {
        match state.engine.scenario().meta.get(sec_str) {
            Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "{}".into()),
            None => "{}".to_string(),
        }
    };

    match CString::new(json_str) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null(),
    }
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_save(h: *const FfiState) -> *const c_char {
    if h.is_null() {
        return ptr::null();
    }
    let state = unsafe { &*h };
    let json = save::save(&state.engine);
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null(),
    }
}

/// # Safety
/// `yaml_ptr` must be a valid pointer to a UTF-8 string of `yaml_len` bytes.
/// `save_json` must be a valid null-terminated C string containing save data.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_restore(
    yaml_ptr: *const c_char,
    yaml_len: c_uint,
    save_json: *const c_char,
) -> *mut FfiState {
    if yaml_ptr.is_null() || save_json.is_null() {
        return ptr::null_mut();
    }

    let slice = unsafe { std::slice::from_raw_parts(yaml_ptr as *const u8, yaml_len as usize) };
    let yaml_str = match std::str::from_utf8(slice) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let scenario = match parser::parse(yaml_str) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let save_str = match unsafe { CStr::from_ptr(save_json) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let registry = Registry::new();
    let engine = match save::restore(scenario, registry, save_str) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };

    let state = Box::new(FfiState {
        engine,
        exec_args: ExecArgs::new(),
        _node_type: None,
        _node_json: None,
        _elements: Vec::new(),
        _element_strings: Vec::new(),
    });

    Box::into_raw(state)
}

/// # Safety
/// `h` must be a valid pointer returned by `yamamva_load` or `yamamva_restore`.
/// After calling this function, `h` must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_free(h: *mut FfiState) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h)); }
    }
}

/// # Safety
/// `s` must be a pointer previously returned by `yamamva_get_state`, `yamamva_meta`,
/// or `yamamva_save`. After calling this function, `s` must not be used again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn yamamva_free_string(s: *const c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s as *mut c_char)); }
    }
}
