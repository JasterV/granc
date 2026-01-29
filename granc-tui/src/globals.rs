use once_cell::sync::OnceCell;
use tokio::runtime::Handle;

pub static TOKIO_HANDLE: OnceCell<Handle> = OnceCell::new();

pub fn init_handle() {
    let handle = Handle::current();
    TOKIO_HANDLE
        .set(handle)
        .expect("Failed to set global Tokio handle");
}

pub fn get_handle() -> &'static Handle {
    TOKIO_HANDLE.get().expect("Tokio handle not initialized")
}
