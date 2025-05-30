use rand::Rng;

#[derive(Debug, Clone)]
struct CredentialError;

/*
	Protocol overview: 
	USER: “POST register/” credential_sign_request
“create_sign_request” sends a credential request with random fields “nym_id” 
SERVER: credential_sign_response
Set the attributes “age” and “measurement_count” in the credential request
Re-randomize attributes “nym_id”
Sign the credential and send it over
	USER: credential
Check that the public attributes “age” and “measurement_count” are set correctly 
“Unblind the credential and store it permanently.
*/

pub fn create_sign_request(nym_id: &[u8]) -> Result<Vec<u8>, CredentialError> {
    // creates a sign_request with random fields `nym_id`
    let mut rng = rand::thread_rng();
    let sign_request: Vec<u8> = (0..nym_id.len()).map(|_| rng.gen()).collect();
    sign_request
}

pub fn create_sign_response(sign_request: &[u8]) -> Result<Vec<u8>, CredentialError> {
    // Set the attributes “age” and “measurement_count” in the credential request
    // Re-randomize attributes “nym_id”
    // Sign the credential and send it over
    let mut rng = rand::thread_rng();
    let blinded_sign_response: Vec<u8> = (0..sign_request.len()).map(|_| rng.gen()).collect();
    blinded_sign_response
}

// Q(michele): what's the key we need in here to unblind this?
pub fn unblind_sign_response(sign_response: &[u8]) -> Result<Vec<u8>, CredentialError> {
    // “Unblind the credential and store it permanently.
    let mut rng = rand::thread_rng();
    let sign_response: Vec<u8> = (0..sign_response.len()).map(|_| rng.gen()).collect();
    sign_response
}

/*
USER: “POST submit/” submission, presentation_message
Compute NYM = PRF(nym_id, nym_scope)
Let measurement_count_msb = floor(UserAuthCredential.measurement_count / 2^10) 
# leak just the most significant bits of the counter
Let age_msb = floor(UserAuthCredential.age / 2^10)
# leak just the most significant bits of the probe age
Compute a credential signing request with:
new.nym_id = old.nym_id
new.age = old.age
new.measurement_count = old.measurement_count + 1
Send 
Measurement
NYM, age_msb, measurement_count_msb
presentation_message for the predicate: 
NYM is correctly computed
PRF(UserAuthCredential.nym_id, nym_scope) = NYM
age_msb and measurement_count are correct
UserAuthCredential.age = age_msb * 2^10 + age_lsb
UserAuthCredential.measurement_count = measurement_count_msb * 2^10 + measurement_count_lsb
age_lsb < 2^10
measurement_count_lsb < 2^10 

	SERVER: decision
Check the credential presentation_message against NYM, TOKEN, age_msb, measurement_count_msb
Check NYM not in blocklist
Add submission.NYM = NYM
Add submission.measurement_count_msb = measurement_count_msb
Add submission to log
Respond with credential_sign_response

	User: credential update
Finalize credential_sign_response and store UserAuthCredential 
*/

pub struct SubmissionMeta {
    nym: Vec<u8>
    age_msb: u32
    measurement_count_msb: u32
}

// Measurement
// NYM, TOKEN, age_msb, measurement_count_msb
pub fn create_submission(nym_id: &[u8], nym_scope: &[u8], current_credential: &[u8]) -> Result<SubmissionMeta, CredentialError> {
    let mut nym = Vec::new();
    nym.extend_from_slice(nym_id);
    nym.extend_from_slice(nym_scope);
    nym.extend_from_slice(current_credential);

    SubmissionMeta {
        nym,
        age_msb: 0,
        measurement_count_msb: 0,
    }
}

pub fn validate_submission(submission_meta: SubmissionMeta) -> Result<Vec<u8>, CredentialError> {
    let mut rng = rand::thread_rng();
    let credential_sign_response: Vec<u8> = (0..submission_meta.nym.len()).map(|_| rng.gen()).collect();
    credential_sign_response
}