use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jbyteArray, jlong};
use jni::JNIEnv;
use serde_json;

use crate::client::{Client, ClientBuilder, ClientOption};

fn throw_runtime_exception(env: &mut JNIEnv, message: &str) -> JString {
    let exception_class = env.find_class("java/lang/RuntimeException").unwrap();
    let message = env.new_string(message).unwrap();
    env.throw_new(exception_class, message.as_str()).unwrap();
    env.new_string("").unwrap().into()
}

#[no_mangle]
pub extern "system" fn Java_OoniProbeClient_createBuilder(_env: JNIEnv, _class: JClass) -> jlong {
    let builder = Box::new(ClientBuilder::new());
    Box::into_raw(builder) as jlong
}

#[no_mangle]
pub extern "system" fn Java_OoniProbeClient_setOption<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    builder_ptr: jlong,
    option_json: JString<'local>,
) -> jlong {
    if builder_ptr == 0 {
        return 0;
    }

    let option_str: String = env
        .get_string(&option_json)
        .expect("Couldn't get java string!")
        .into();

    let option: ClientOption = match serde_json::from_str(&option_str) {
        Ok(opt) => opt,
        Err(_) => return 0,
    };

    unsafe {
        let builder = &mut *(builder_ptr as *mut ClientBuilder);
        builder.set_option(option);
        builder_ptr
    }
}

#[no_mangle]
pub extern "system" fn Java_OoniProbeClient_build(
    _env: JNIEnv,
    _class: JClass,
    builder_ptr: jlong,
) -> jlong {
    if builder_ptr == 0 {
        return 0;
    }

    unsafe {
        let builder = Box::from_raw(builder_ptr as *mut ClientBuilder);
        match builder.build() {
            Ok(client) => Box::into_raw(Box::new(client)) as jlong,
            Err(_) => 0,
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_OoniProbeClient_destroyBuilder(
    _env: JNIEnv,
    _class: JClass,
    builder_ptr: jlong,
) {
    if builder_ptr != 0 {
        unsafe {
            // by making _ go out of scope we are deallocating the ClientBuilder
            let _ = Box::from_raw(builder_ptr as *mut ClientBuilder);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_OoniProbeClient_destroyClient(
    _env: JNIEnv,
    _class: JClass,
    client_ptr: jlong,
) {
    if client_ptr != 0 {
        unsafe {
            // by making _ go out of scope we are deallocating the Client
            let _ = Box::from_raw(client_ptr as *mut Client);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_OoniProbeClient_get<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    client_ptr: jlong,
    url: JString<'local>,
) -> JString<'local> {
    if client_ptr == 0 {
        return std::ptr::null_mut();
    }

    let url: String = env
        .get_string(&url)
        .expect("Couldn't get java string!")
        .into();

    let client = unsafe { &*(client_ptr as *const Client) };

    match client.get(&url) {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(json_str) => env.new_string(json_str).unwrap().into(),
            Err(e) => {
                throw_runtime_exception(&mut env, &format!("JSON serialization error: {}", e))
            }
        },
        Err(e) => throw_runtime_exception(&mut env, &format!("Request failed: {:?}", e)),
    }
    match result {
        Some(json_str) => env.new_string(json_str).unwrap().into(),
        None => env.new_string("").unwrap().into(),
    }
}
