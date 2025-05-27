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

pub fn create_sign_request(nym_id: String) -> String {
    return sprintf!("xxxx");
}

pub fn create_sign_response(sign_request: String) -> String {
    return sprintf!("xxxx");
}

pub fn unblind_sign_response(sign_response: String) -> String {
    return sprintf!("xxxx");
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
NYM, TOKEN, age_msb, measurement_count_msb
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

enum NymScope {
}

pub struct SubmissionMeta {
    nym: String
    token: String
    age_msb: u32
    measurement_count_msb: u32
}

// Measurement
// NYM, TOKEN, age_msb, measurement_count_msb
pub fn create_submission(
    nym_id: String,
    nym_scope: NymScope,
    current_credential: String
) -> SubmissionMeta {
    return SubmissionMeta{}
}

pub fn validate_submission(submission_meta: SubmissionMeta) -> String {
    return sprintf!("XXXX")
}