use std::cell::RefCell;

// push 阶段的授权头（线程局部）。值应为完整的值，如 "Basic base64(user:pass)"。
thread_local! { static PUSH_AUTH: RefCell<Option<String>> = RefCell::new(None); }

pub fn set_push_auth_header_value(v: Option<String>) {
    PUSH_AUTH.with(|h| {
        *h.borrow_mut() = v;
    });
}

pub(super) fn get_push_auth_header() -> Option<String> {
    PUSH_AUTH.with(|h| h.borrow().clone())
}
