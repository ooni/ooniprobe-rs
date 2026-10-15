pub mod capi;
pub mod client;
pub mod errors;
pub mod userauth;

pub use client::{client_get, client_post, client_put, HttpResponse, KeyValue};
pub use errors::OoniError;
pub use userauth::{
    get_probe_id, protocol_version, userauth_register, userauth_submit, CredentialConfig,
    CredentialResult, ParamRange, ProbeIDResult,
};

// Required for UniFFI scaffolding
uniffi::include_scaffolding!("ooniprobe");
