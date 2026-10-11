//! Partial configuration updates preserve every omitted field.
use serde::{Deserialize, Serialize};
macro_rules! partial {
    ($name:ident,$target:ty,{$($field:ident:$ty:ty),*$(,)?})=>{
        #[derive(Debug,Clone,Default,PartialEq,Eq,Serialize,Deserialize)]
        #[serde(default,deny_unknown_fields)]
        pub struct $name { $(#[serde(skip_serializing_if="Option::is_none")]pub $field:Option<$ty>,)* }
        impl $name { pub fn apply_to(&self,target:&mut $target){$(if let Some(value)=&self.$field{target.$field=value.clone();})*} }
        impl From<$target> for $name {fn from(value:$target)->Self{Self{$($field:Some(value.$field),)*}}}
    }
}
partial!(FeaturesPatch,crate::policy::feature_gate::FeatureFlags,{save_text_file:bool,chmod_file:bool,extract_archive:bool,delete_permanently:bool,chown:bool});
partial!(LimitsPatch,crate::limits::Limits,{max_frame_payload:usize,max_control_payload:usize,max_decompressed_control_payload:usize,max_chunk_size:usize,max_in_flight_upload:usize,max_directory_page_size:usize,default_directory_page_size:usize,max_text_edit_size:usize,max_preview_size:usize});
partial!(SessionPatch,crate::config::SessionConfig,{cookie_name:String,idle_timeout_seconds:u64,absolute_timeout_seconds:u64,secure_cookies:bool,same_site:crate::config::SameSitePolicy});
partial!(SecurityPatch,crate::config::SecurityConfig,{production_wire_protocol:crate::config::WireProtocol,allow_plain_ws_localhost_only:bool,allowed_origins:Vec<String>});
partial!(RateLimitPatch,crate::config::RateLimitConfig,{max_login_failures:u32,lockout_duration_seconds:u64});
partial!(HelperPatch,crate::config::HelperConfig,{timeout_seconds:u64});
partial!(SearchPatch,crate::config::SearchConfig,{max_depth:usize,default_max_results:usize,max_results_limit:usize});
partial!(ProxyPatch,crate::client_ip::ProxyConfig,{trusted_proxies:Vec<String>,client_ip_header:crate::client_ip::ClientIpHeader});
pub fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuditPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<std::path::PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_archives: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "nullable")]
    pub retention_days: Option<Option<u32>>,
}
impl AuditPatch {
    pub fn apply_to(&self, target: &mut crate::config::AuditConfig) {
        if let Some(path) = &self.path {
            target.path = Some(path.clone());
        }
        if let Some(value) = self.max_file_bytes {
            target.max_file_bytes = value;
        }
        if let Some(value) = self.max_archives {
            target.max_archives = value;
        }
        if let Some(value) = self.retention_days {
            target.retention_days = value.filter(|days| *days != 0);
        }
    }
}
