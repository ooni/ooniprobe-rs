use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jbyteArray, jlong};
use jni::JNIEnv;
use serde_json;

use ooniprobe_services::client::{Client, ClientBuilder, ClientOptions};

#[no_mangle]
pub extern "system" fn Java_main_OoniProbeClient_00024Builder_createBuilder(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let builder = Box::new(Client::builder());
    Box::into_raw(builder) as jlong
}

#[no_mangle]
pub extern "system" fn Java_main_OoniProbeClient_00024Builder_setOptions<'local>(
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

    let options: ClientOptions = match serde_json::from_str(&option_str) {
        Ok(opt) => opt,
        Err(_) => return 0,
    };

    unsafe {
        let builder = Box::from_raw(builder_ptr as *mut ClientBuilder);
        Box::into_raw(Box::new(builder.set_options(options))) as jlong
    }
}

#[no_mangle]
pub extern "system" fn Java_main_OoniProbeClient_00024Builder_buildClient(
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
pub extern "system" fn Java_main_OoniProbeClient_destroyClient(
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
pub extern "system" fn Java_main_OoniProbeClient_00024Request_execute<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    client_ptr: jlong,
    request_builder_ptr: jlong,
) -> JString<'local> {
    if client_ptr == 0 {
        env.throw("invalid client_ptr");
        unreachable!();
    }
    if request_builder_ptr == 0 {
        env.throw("invalid request_ptr");
        unreachable!();
    }

    let client = unsafe { &*(client_ptr as *const Client) };
    let request_builder =
        unsafe { Box::from_raw(request_builder_ptr as *mut rquest::RequestBuilder) };

    let request = match request_builder.build() {
        Ok(v) => v,
        Err(_) => {
            env.throw("failed to build request");
            unreachable!()
        }
    };

    let response = match client.execute(request) {
        Ok(v) => v,
        Err(_) => {
            env.throw("failed to perform request");
            unreachable!()
        }
    };

    let json_string = match response.to_json_str() {
        Ok(v) => v,
        Err(_) => {
            env.throw("failed to serialize string");
            unreachable!()
        }
    };
    env.new_string(json_string).unwrap().into()
}

#[no_mangle]
pub extern "system" fn Java_main_OoniProbeClient_request<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    client_ptr: jlong,
    method: JString<'local>,
    url: JString<'local>,
) -> jlong {
    if client_ptr == 0 {
        env.throw("invalid request_ptr");
        return 0;
    }

    let client = unsafe { &*(client_ptr as *const Client) };

    let method: String = env
        .get_string(&method)
        .expect("Couldn't get method string!")
        .into();

    let url: String = env
        .get_string(&url)
        .expect("Couldn't get method string!")
        .into();

    let request_builder = match client.request(method.as_str(), url.as_str()) {
        Ok(v) => v,
        Err(_) => {
            env.throw("unable to build request");
            unreachable!()
        }
    };
    Box::into_raw(Box::new(request_builder)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_main_OoniProbeClient_00024Request_addHeader<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    request_builder_ptr: jlong,
    name: JString<'local>,
    value: JString<'local>,
) -> jlong {
    if request_builder_ptr == 0 {
        env.throw("invalid request_builder_ptr");
        return 0;
    }

    let request_builder =
        unsafe { Box::from_raw(request_builder_ptr as *mut rquest::RequestBuilder) };

    let name: String = env
        .get_string(&name)
        .expect("Couldn't get header_name string!")
        .into();

    let value: String = env
        .get_string(&value)
        .expect("Couldn't get header_value string!")
        .into();

    Box::into_raw(Box::new(request_builder.header_append(name, value))) as jlong
}
