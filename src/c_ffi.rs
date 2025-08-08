use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use crate::{HarmonyEncoding, load_harmony_encoding, HarmonyEncodingName, StreamableParser};
use crate::chat::{Conversation, Message, Role, SystemContent};

// Opaque pointers for Rust types
pub struct HarmonyEncodingWrapper {
    encoding: HarmonyEncoding,
}

pub struct StreamableParserWrapper {
    #[allow(dead_code)] // Reserved for future streaming functionality
    parser: StreamableParser,
}

// Error handling
#[repr(C)]
pub struct HarmonyResult {
    success: bool,
    error_message: *mut c_char,
}

impl HarmonyResult {
    fn ok() -> Self {
        HarmonyResult {
            success: true,
            error_message: ptr::null_mut(),
        }
    }
    
    fn err(msg: String) -> Self {
        let c_string = CString::new(msg).unwrap_or_else(|_| CString::new("Error").unwrap());
        HarmonyResult {
            success: false,
            error_message: c_string.into_raw(),
        }
    }
}

// Free functions
#[no_mangle]
pub extern "C" fn harmony_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { 
            let _ = CString::from_raw(s);
        }
    }
}

#[no_mangle]
pub extern "C" fn harmony_free_tokens(tokens: *mut u32, len: usize) {
    if !tokens.is_null() {
        unsafe {
            let _ = Vec::from_raw_parts(tokens, len, len);
        }
    }
}

// Harmony Encoding functions
#[no_mangle]
pub extern "C" fn harmony_encoding_new() -> *mut HarmonyEncodingWrapper {
    match load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss) {
        Ok(encoding) => {
            let wrapper = Box::new(HarmonyEncodingWrapper { encoding });
            Box::into_raw(wrapper)
        }
        Err(_) => ptr::null_mut()
    }
}

#[no_mangle]
pub extern "C" fn harmony_encoding_free(wrapper: *mut HarmonyEncodingWrapper) {
    if !wrapper.is_null() {
        unsafe {
            let _ = Box::from_raw(wrapper);
        }
    }
}

// Plain text encoding - encode text without Harmony formatting
#[no_mangle]
pub extern "C" fn harmony_encoding_encode_plain(
    wrapper: *const HarmonyEncodingWrapper,
    text: *const c_char,
    tokens_out: *mut *mut u32,
    tokens_len: *mut usize,
) -> HarmonyResult {
    if wrapper.is_null() {
        return HarmonyResult::err("Null encoding wrapper".to_string());
    }
    
    let encoding = unsafe { &(*wrapper).encoding };
    
    // Get text string
    if text.is_null() {
        return HarmonyResult::err("Null text".to_string());
    }
    
    let text_str = unsafe { CStr::from_ptr(text) }
        .to_str()
        .unwrap_or("");
    
    // Use the official harmony tokenizer for plain encoding
    let tokens = encoding.tokenizer.encode_ordinary(text_str);
    
    // Convert to raw pointer
    let mut tokens_vec = tokens;
    tokens_vec.shrink_to_fit();
    let len = tokens_vec.len();
    let ptr = tokens_vec.as_mut_ptr();
    std::mem::forget(tokens_vec);
    
    unsafe {
        *tokens_len = len;
        *tokens_out = ptr;
    }
    
    HarmonyResult::ok()
}

// Harmony prompt rendering
#[no_mangle]
pub extern "C" fn harmony_encoding_render_prompt(
    wrapper: *const HarmonyEncodingWrapper,
    system_msg: *const c_char,
    user_msg: *const c_char,
    assistant_prefix: *const c_char,
    tokens_out: *mut *mut u32,
    tokens_len: *mut usize,
) -> HarmonyResult {
    if wrapper.is_null() {
        return HarmonyResult::err("Null encoding wrapper".to_string());
    }
    
    let encoding = unsafe { &(*wrapper).encoding };
    
    let mut messages = Vec::new();
    
    // Add system message if provided
    if !system_msg.is_null() {
        let system_text = unsafe { CStr::from_ptr(system_msg) }
            .to_str()
            .unwrap_or("");
        
        if !system_text.is_empty() {
            // Create a system message with the text as model_identity
            let system_content = SystemContent::new().with_model_identity(system_text);
            let message = Message::from_role_and_content(Role::System, system_content);
            messages.push(message);
        }
    }
    
    // Add user message
    if user_msg.is_null() {
        return HarmonyResult::err("Null user message".to_string());
    }
    
    let user_text = unsafe { CStr::from_ptr(user_msg) }
        .to_str()
        .unwrap_or("");
    
    let user_message = Message::from_role_and_content(Role::User, user_text.to_string());
    messages.push(user_message);
    
    // Add assistant prefix if provided
    if !assistant_prefix.is_null() {
        let assistant_text = unsafe { CStr::from_ptr(assistant_prefix) }
            .to_str()
            .unwrap_or("");
        
        if !assistant_text.is_empty() {
            let assistant_message = Message::from_role_and_content(Role::Assistant, assistant_text.to_string());
            messages.push(assistant_message);
        }
    }
    
    // Create conversation and render it
    let conversation = Conversation::from_messages(messages);
    match encoding.render_conversation(&conversation, None) {
        Ok(tokens) => {
            // Convert to raw pointer
            let mut tokens_vec = tokens;
            tokens_vec.shrink_to_fit();
            let len = tokens_vec.len();
            let ptr = tokens_vec.as_mut_ptr();
            std::mem::forget(tokens_vec);
            
            unsafe {
                *tokens_len = len;
                *tokens_out = ptr;
            }
            
            HarmonyResult::ok()
        }
        Err(e) => HarmonyResult::err(format!("Failed to render conversation: {}", e))
    }
}

// Decode tokens to text
#[no_mangle]
pub extern "C" fn harmony_encoding_decode(
    wrapper: *const HarmonyEncodingWrapper,
    tokens: *const u32,
    tokens_len: usize,
) -> *mut c_char {
    if wrapper.is_null() || tokens.is_null() {
        return ptr::null_mut();
    }
    
    let encoding = unsafe { &(*wrapper).encoding };
    let tokens_slice = unsafe { std::slice::from_raw_parts(tokens, tokens_len) };
    
    match encoding.tokenizer.decode_bytes(tokens_slice) {
        Ok(bytes) => {
            match String::from_utf8(bytes) {
                Ok(text) => {
                    match CString::new(text) {
                        Ok(c_str) => c_str.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

// Get stop tokens
#[no_mangle]
pub extern "C" fn harmony_encoding_stop_tokens(
    wrapper: *const HarmonyEncodingWrapper,
    tokens_out: *mut *mut u32,
    tokens_len: *mut usize,
) -> HarmonyResult {
    if wrapper.is_null() {
        return HarmonyResult::err("Null encoding wrapper".to_string());
    }
    
    let encoding = unsafe { &(*wrapper).encoding };
    
    // Get stop tokens from the encoding
    let stop_tokens = match encoding.stop_tokens() {
        Ok(tokens) => tokens.into_iter().collect::<Vec<_>>(),
        Err(e) => return HarmonyResult::err(format!("Failed to get stop tokens: {}", e)),
    };
    
    // Convert to raw pointer
    let mut tokens_vec = stop_tokens;
    tokens_vec.shrink_to_fit();
    let len = tokens_vec.len();
    let ptr = tokens_vec.as_mut_ptr();
    std::mem::forget(tokens_vec);
    
    unsafe {
        *tokens_len = len;
        *tokens_out = ptr;
    }
    
    HarmonyResult::ok()
}