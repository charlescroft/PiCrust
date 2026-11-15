//! In-process JavaScript sandbox for Code Mode (`pi-gpui/src/engine/quickjs_sandbox.rs`).
//!
//! On macOS, links directly against Apple's built-in `JavaScriptCore.framework` C API
//! (`JSContextGroupCreate`, `JSGlobalContextCreateInGroup`, `JSEvaluateScript`,
//! `JSObjectMakeFunctionWithCallback`) to execute Code Mode scripts in-process with zero
//! Node.js subprocess overhead and < 1ms context creation time.
//! On other platforms, provides a clean fallback interface.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::engine::native_tools::{execute_edit_tool, execute_read_tool, execute_write_tool};

/// Output returned by an in-process Code Mode JS sandbox execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsSandboxOutput {
    pub result: Value,
    pub logs: Vec<String>,
    pub is_error: bool,
    pub error_message: Option<String>,
}

struct HostContext {
    cwd: PathBuf,
    logs: Vec<String>,
}

thread_local! {
    static ACTIVE_HOST_CTX: Mutex<Option<HostContext>> = const { Mutex::new(None) };
}

/// Executes a JavaScript snippet inside an isolated in-process JS context with host bindings:
/// - `console.log(...)`
/// - `pi.read({ path, offset?, limit? })`
/// - `pi.write({ path, content })`
/// - `pi.edit({ path, oldText, newText })`
pub fn eval_codemode_script(cwd: &Path, script: &str) -> JsSandboxOutput {
    ACTIVE_HOST_CTX.with(|slot| {
        *slot.lock().unwrap() = Some(HostContext {
            cwd: cwd.to_path_buf(),
            logs: Vec::new(),
        });
    });

    let outcome = eval_js_platform(script);

    let logs = ACTIVE_HOST_CTX.with(|slot| {
        slot.lock()
            .unwrap()
            .take()
            .map(|c| c.logs)
            .unwrap_or_default()
    });

    match outcome {
        Ok(val) => JsSandboxOutput {
            result: val,
            logs,
            is_error: false,
            error_message: None,
        },
        Err(err) => JsSandboxOutput {
            result: Value::Null,
            logs,
            is_error: true,
            error_message: Some(err),
        },
    }
}

fn dispatch_host_call(op: &str, args_json: &str) -> String {
    let parsed: Value = serde_json::from_str(args_json).unwrap_or_else(|_| json!({}));
    ACTIVE_HOST_CTX.with(|slot| {
        let mut guard = slot.lock().unwrap();
        let Some(ctx) = guard.as_mut() else {
            return json!({ "error": "No active host context" }).to_string();
        };
        match op {
            "log" => {
                if let Some(msg) = parsed.get("message").and_then(Value::as_str) {
                    ctx.logs.push(msg.to_string());
                } else {
                    ctx.logs.push(parsed.to_string());
                }
                json!({ "ok": true }).to_string()
            }
            "read" => {
                let res = execute_read_tool(&ctx.cwd, &parsed);
                serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
            }
            "write" => {
                let res = execute_write_tool(&ctx.cwd, &parsed);
                serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
            }
            "edit" => {
                let res = execute_edit_tool(&ctx.cwd, &parsed);
                serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
            }
            _ => json!({ "is_error": true, "output": format!("Unknown host op: {op}") }).to_string(),
        }
    })
}

#[cfg(target_os = "macos")]
mod macos_jsc {
    use super::*;
    use std::ffi::{CStr, CString, c_char, c_void};
    use std::ptr;

    type JSContextGroupRef = *const c_void;
    type JSGlobalContextRef = *mut c_void;
    type JSContextRef = *const c_void;
    type JSObjectRef = *mut c_void;
    type JSValueRef = *const c_void;
    type JSStringRef = *mut c_void;
    type JSClassRef = *const c_void;

    type JSObjectCallAsFunctionCallback = unsafe extern "C" fn(
        ctx: JSContextRef,
        function: JSObjectRef,
        this_object: JSObjectRef,
        argument_count: usize,
        arguments: *const JSValueRef,
        exception: *mut JSValueRef,
    ) -> JSValueRef;

    #[link(name = "JavaScriptCore", kind = "framework")]
    unsafe extern "C" {
        fn JSContextGroupCreate() -> JSContextGroupRef;
        fn JSContextGroupRelease(group: JSContextGroupRef);
        fn JSGlobalContextCreateInGroup(
            group: JSContextGroupRef,
            global_object_class: JSClassRef,
        ) -> JSGlobalContextRef;
        fn JSGlobalContextRelease(ctx: JSGlobalContextRef);
        fn JSContextGetGlobalObject(ctx: JSContextRef) -> JSObjectRef;

        fn JSStringCreateWithUTF8CString(string: *const c_char) -> JSStringRef;
        fn JSStringRelease(string: JSStringRef);
        fn JSStringGetMaximumUTF8CStringSize(string: JSStringRef) -> usize;
        fn JSStringGetUTF8CString(string: JSStringRef, buffer: *mut c_char, buffer_size: usize) -> usize;

        fn JSValueMakeString(ctx: JSContextRef, string: JSStringRef) -> JSValueRef;
        fn JSValueMakeUndefined(ctx: JSContextRef) -> JSValueRef;
        fn JSValueToStringCopy(
            ctx: JSContextRef,
            value: JSValueRef,
            exception: *mut JSValueRef,
        ) -> JSStringRef;
        fn JSValueCreateJSONString(
            ctx: JSContextRef,
            value: JSValueRef,
            indent: u32,
            exception: *mut JSValueRef,
        ) -> JSStringRef;

        fn JSObjectMakeFunctionWithCallback(
            ctx: JSContextRef,
            name: JSStringRef,
            call_as_function: JSObjectCallAsFunctionCallback,
        ) -> JSObjectRef;
        fn JSObjectSetProperty(
            ctx: JSContextRef,
            object: JSObjectRef,
            property_name: JSStringRef,
            value: JSValueRef,
            attributes: u32,
            exception: *mut JSValueRef,
        );

        fn JSEvaluateScript(
            ctx: JSContextRef,
            script: JSStringRef,
            this_object: JSObjectRef,
            source_url: JSStringRef,
            starting_line_number: i32,
            exception: *mut JSValueRef,
        ) -> JSValueRef;
    }

    unsafe fn js_string_to_rust(js_str: JSStringRef) -> String {
        if js_str.is_null() {
            return String::new();
        }
        unsafe {
            let max_sz = JSStringGetMaximumUTF8CStringSize(js_str);
            if max_sz == 0 {
                return String::new();
            }
            let mut buf = vec![0u8; max_sz];
            let written = JSStringGetUTF8CString(js_str, buf.as_mut_ptr().cast::<c_char>(), max_sz);
            if written == 0 {
                return String::new();
            }
            let cstr = CStr::from_bytes_until_nul(&buf).unwrap_or_default();
            cstr.to_string_lossy().into_owned()
        }
    }

    unsafe fn js_value_to_string(ctx: JSContextRef, val: JSValueRef) -> String {
        if val.is_null() {
            return String::new();
        }
        unsafe {
            let mut exc: JSValueRef = ptr::null();
            let s = JSValueToStringCopy(ctx, val, &raw mut exc);
            let out = js_string_to_rust(s);
            if !s.is_null() {
                JSStringRelease(s);
            }
            out
        }
    }

    unsafe extern "C" fn host_dispatch_callback(
        ctx: JSContextRef,
        _function: JSObjectRef,
        _this_object: JSObjectRef,
        argument_count: usize,
        arguments: *const JSValueRef,
        _exception: *mut JSValueRef,
    ) -> JSValueRef {
        unsafe {
            if argument_count < 2 || arguments.is_null() {
                return JSValueMakeUndefined(ctx);
            }
            let args = std::slice::from_raw_parts(arguments, argument_count);
            let op = js_value_to_string(ctx, args[0]);
            let payload = js_value_to_string(ctx, args[1]);
            let response = dispatch_host_call(&op, &payload);
            let c_resp = CString::new(response).unwrap_or_default();
            let js_resp = JSStringCreateWithUTF8CString(c_resp.as_ptr());
            let val = JSValueMakeString(ctx, js_resp);
            JSStringRelease(js_resp);
            val
        }
    }

    pub fn eval_js(script: &str) -> Result<Value, String> {
        unsafe {
            let group = JSContextGroupCreate();
            let ctx = JSGlobalContextCreateInGroup(group, ptr::null());
            let global = JSContextGetGlobalObject(ctx);

            let fn_name_c = CString::new("__pi_host_call").unwrap();
            let fn_name = JSStringCreateWithUTF8CString(fn_name_c.as_ptr());
            let func_obj = JSObjectMakeFunctionWithCallback(ctx, fn_name, host_dispatch_callback);
            let mut exc: JSValueRef = ptr::null();
            JSObjectSetProperty(ctx, global, fn_name, func_obj, 0, &raw mut exc);
            JSStringRelease(fn_name);

            let bootstrap = r#"
                const console = {
                    log: (...args) => {
                        const msg = args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ');
                        __pi_host_call('log', JSON.stringify({ message: msg }));
                    },
                    warn: (...args) => console.log(...args),
                    error: (...args) => console.log(...args),
                };
                const pi = {
                    read: (args) => JSON.parse(__pi_host_call('read', JSON.stringify(args || {}))),
                    write: (args) => JSON.parse(__pi_host_call('write', JSON.stringify(args || {}))),
                    edit: (args) => JSON.parse(__pi_host_call('edit', JSON.stringify(args || {}))),
                };
            "#;

            let full_script = format!("{bootstrap}\n{script}");
            let script_c = CString::new(full_script).map_err(|e| e.to_string())?;
            let script_js = JSStringCreateWithUTF8CString(script_c.as_ptr());
            let mut eval_exc: JSValueRef = ptr::null();
            let result_val = JSEvaluateScript(ctx, script_js, ptr::null_mut(), ptr::null_mut(), 1, &raw mut eval_exc);
            JSStringRelease(script_js);

            if !eval_exc.is_null() {
                let err_str = js_value_to_string(ctx, eval_exc);
                JSGlobalContextRelease(ctx);
                JSContextGroupRelease(group);
                return Err(err_str);
            }

            let mut json_exc: JSValueRef = ptr::null();
            let json_js = JSValueCreateJSONString(ctx, result_val, 0, &raw mut json_exc);
            let out_val = if !json_js.is_null() && json_exc.is_null() {
                let json_str = js_string_to_rust(json_js);
                JSStringRelease(json_js);
                serde_json::from_str::<Value>(&json_str).unwrap_or(Value::String(json_str))
            } else {
                let fallback = js_value_to_string(ctx, result_val);
                if fallback == "undefined" || fallback.is_empty() {
                    Value::Null
                } else {
                    Value::String(fallback)
                }
            };

            JSGlobalContextRelease(ctx);
            JSContextGroupRelease(group);
            Ok(out_val)
        }
    }
}

#[cfg(target_os = "macos")]
fn eval_js_platform(script: &str) -> Result<Value, String> {
    macos_jsc::eval_js(script)
}

#[cfg(not(target_os = "macos"))]
fn eval_js_platform(_script: &str) -> Result<Value, String> {
    Err("In-process JS sandbox requires macOS JavaScriptCore or embedded QuickJS".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_js_and_invokes_host_file_tools_in_process() {
        if !cfg!(target_os = "macos") {
            // In-process JavaScriptCore framework is currently macOS-exclusive
            return;
        }
        let dir = std::env::temp_dir().join(format!("pi-gpui-jsc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let script = r#"
            console.log("starting codemode");
            pi.write({ path: "sample.txt", content: "alpha\nbeta\n" });
            pi.edit({ path: "sample.txt", oldText: "beta", newText: "GAMMA" });
            const r = pi.read({ path: "sample.txt" });
            ({ output: r.output, sum: 21 * 2 })
        "#;

        let out = eval_codemode_script(&dir, script);
        assert!(!out.is_error, "unexpected error: {:?}", out.error_message);
        assert_eq!(out.logs, vec!["starting codemode".to_string()]);
        assert_eq!(out.result["sum"], json!(42));
        assert_eq!(out.result["output"], json!("alpha\nGAMMA"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
