//! Built-in assets remain embedded; edit their individual source files.

pub const INDEX_HTML: &str = include_str!("static/index.html");
pub const STYLE_CSS: &str = include_str!("static/style.css");
pub const FAVICON_SVG: &str = include_str!("static/favicon.svg");
pub const APP_JS: &str = include_str!("static/app.js");

pub const ACCOUNTS_JS: &str = include_str!("static/accounts.js");
