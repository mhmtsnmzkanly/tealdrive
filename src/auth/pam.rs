use std::ffi::{CString, c_void};
use crate::auth::account::{AuthError, AuthRequest, AuthSuccess, Authenticator};
use crate::config::AccountPolicyConfig;

#[derive(Debug, Clone)]
pub struct PamAuthenticator { helper: Option<crate::helper::client::HelperProcessClient> }
impl PamAuthenticator {
    pub fn new() -> Self { Self { helper: None } }
    pub fn with_helper(helper: crate::helper::client::HelperProcessClient) -> Self { Self { helper: Some(helper) } }
}
impl Default for PamAuthenticator { fn default() -> Self { Self::new() } }
impl Authenticator for PamAuthenticator {
    fn authenticate(&self, request: &AuthRequest) -> Result<AuthSuccess, AuthError> {
        match &self.helper { Some(helper) => helper.authenticate(request), None => authenticate_local(request, &AccountPolicyConfig::default()) }
    }
}
#[repr(C)] struct PamMessage { style: libc::c_int, message: *const libc::c_char }
#[repr(C)] struct PamResponse { response: *mut libc::c_char, code: libc::c_int }
#[repr(C)] struct PamConversation {
    callback: unsafe extern "C" fn(libc::c_int,*mut *const PamMessage,*mut *mut PamResponse,*mut c_void)->libc::c_int,
    data: *mut c_void,
}
struct Credentials { user: CString, password: CString }
unsafe extern "C" fn converse(n: libc::c_int, messages: *mut *const PamMessage, responses: *mut *mut PamResponse, data: *mut c_void) -> libc::c_int {
    if !(1..=32).contains(&n) || messages.is_null() || responses.is_null() || data.is_null() { return 19; }
    let credentials = &*(data as *const Credentials);
    let array = libc::calloc(n as usize, std::mem::size_of::<PamResponse>()) as *mut PamResponse;
    if array.is_null() { return 19; }
    for i in 0..n as usize {
        let message = *messages.add(i);
        if message.is_null() { libc::free(array.cast()); return 19; }
        let answer = match (*message).style {
            1 => Some(credentials.password.as_ptr()), 2 => Some(credentials.user.as_ptr()), 3 | 4 => None,
            _ => { for j in 0..i { libc::free((*array.add(j)).response.cast()); } libc::free(array.cast()); return 19; }
        };
        if let Some(answer) = answer {
            (*array.add(i)).response = libc::strdup(answer);
            if (*array.add(i)).response.is_null() {
                for j in 0..i { libc::free((*array.add(j)).response.cast()); }
                libc::free(array.cast()); return 19;
            }
        }
    }
    *responses = array; 0
}
pub fn authenticate_local(request: &AuthRequest, policy: &AccountPolicyConfig) -> Result<AuthSuccess, AuthError> {
    if request.password.is_empty() { return Err(AuthError::InvalidCredentials); }
    let mut credentials = Credentials { user: CString::new(request.username.as_str()).map_err(|_|AuthError::InvalidCredentials)?,
        password: CString::new(request.password.as_str()).map_err(|_|AuthError::InvalidCredentials)? };
    unsafe {
        let library = libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
        if library.is_null() { return Err(AuthError::AuthBackendUnavailable); }
        let result = (|| {
            let start = libc::dlsym(library, c"pam_start".as_ptr());
            let authenticate = libc::dlsym(library,c"pam_authenticate".as_ptr());
            let account = libc::dlsym(library,c"pam_acct_mgmt".as_ptr());
            let end = libc::dlsym(library,c"pam_end".as_ptr());
            if [start,authenticate,account,end].iter().any(|p|p.is_null()) { return Err(AuthError::AuthBackendUnavailable); }
            let start: unsafe extern "C" fn(*const libc::c_char,*const libc::c_char,*const PamConversation,*mut *mut c_void)->libc::c_int=std::mem::transmute(start);
            let auth: unsafe extern "C" fn(*mut c_void,libc::c_int)->libc::c_int=std::mem::transmute(authenticate);
            let acct: unsafe extern "C" fn(*mut c_void,libc::c_int)->libc::c_int=std::mem::transmute(account);
            let end: unsafe extern "C" fn(*mut c_void,libc::c_int)->libc::c_int=std::mem::transmute(end);
            let conversation=PamConversation{callback:converse,data:(&mut credentials as *mut Credentials).cast()};
            let mut handle=std::ptr::null_mut();
            let status=start(c"tealdrive".as_ptr(),credentials.user.as_ptr(),&conversation,&mut handle);
            if status!=0 { return Err(AuthError::AuthBackendUnavailable); }
            let status=auth(handle,1); // PAM_DISALLOW_NULL_AUTHTOK
            let status=if status==0 { acct(handle,0) } else { status };
            end(handle,status);
            if status!=0 { return Err(if status==13 { AuthError::AccountExpired } else { AuthError::InvalidCredentials }); }
            crate::helper::security::account(&request.username)
        })();
        libc::dlclose(library);
        let success=result?;
        let candidate=crate::auth::account::MockUser { username:success.username.clone(),password:String::new(),uid:success.uid,primary_gid:success.primary_gid,
            home_dir:success.home_dir.clone(),shell:success.shell.clone(),groups:success.groups.clone(),status:crate::auth::account::AccountStatus::Active };
        match crate::auth::account::evaluate_account_policy(&candidate,policy) {
            crate::auth::account::AccountPolicyResult::Allowed=>Ok(success),
            crate::auth::account::AccountPolicyResult::Rejected(_)=>Err(AuthError::InvalidCredentials),
        }
    }
}
