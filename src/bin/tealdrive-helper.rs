use std::io::{stdin, stdout};

fn main() {
    if std::env::args().any(|arg| arg == "--watch-stdin") {
        tealdrive::helper::cancellation::request_parent_watch();
    }
    let result =
        tealdrive::helper::ipc::run_secure_helper(&mut stdin().lock(), &mut stdout().lock());
    if result.is_err() {
        std::process::exit(1);
    }
}
