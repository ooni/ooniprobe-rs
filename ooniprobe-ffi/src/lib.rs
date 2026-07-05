pub mod capi;
pub mod client;
pub mod errors;
pub mod userauth;

pub use client::{client_get, client_post, HttpResponse, KeyValue};
pub use errors::OoniError;
pub use userauth::{
    get_probe_id, userauth_register, userauth_submit, CredentialConfig, CredentialResult,
    ParamRange, ProbeIDResult,
};

// Required for UniFFI scaffolding
uniffi::include_scaffolding!("ooniprobe");
