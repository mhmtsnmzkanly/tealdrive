//! Root-signed operation ID names private artifacts so maintenance does not trust user metadata.
use std::cell::RefCell;
thread_local! { static ID: RefCell<Option<String>> = const { RefCell::new(None) }; }
pub(super) fn current() -> Option<String> {
    ID.with(|id| id.borrow().clone())
}
pub(super) fn with_operation<T>(id: Option<&str>, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<String>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ID.with(|id| *id.borrow_mut() = self.0.take());
        }
    }
    let _reset = Reset(ID.with(|slot| slot.replace(id.map(str::to_owned))));
    run()
}
