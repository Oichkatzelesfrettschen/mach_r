//! Pager Server (MIG dispatcher)
//!
//! Minimal placeholder pager service that responds to page_request.

use crate::message::Message;
use crate::port::Port;
use crate::types::TaskId;
use alloc::sync::Arc;

pub struct PagerServer {
    port: Arc<Port>,
    server_task: TaskId,
}

impl PagerServer {
    pub fn new(server_task: TaskId) -> Self {
        let port = Port::new(server_task);
        Self { port, server_task }
    }
    pub fn server_port(&self) -> crate::types::PortId {
        self.port.id()
    }
    pub fn server_port_arc(&self) -> Arc<Port> {
        Arc::clone(&self.port)
    }
    pub fn handle_message(&self, msg: Message) -> Option<Message> {
        crate::mig::generated::pager::dispatch(self, &msg)
    }
    pub fn poll_once(&self) {
        if let Some(msg) = self.port.receive() {
            if let Some(reply) = self.handle_message(msg) {
                let _ = crate::port::send_message(reply.remote_port(), reply);
            }
        }
    }
}

impl crate::mig::generated::pager::NameService for PagerServer {
    fn page_request(
        &self,
        _object_id: u64,
        _offset: u64,
        _size: u32,
        _protection: u32,
    ) -> Result<u64, i32> {
        // Placeholder: return a dummy physical address
        Ok(0x10000)
    }
}

pub static mut PAGER_SERVER: Option<PagerServer> = None;

pub fn init() {
    let server_task = TaskId(5);
    let srv = PagerServer::new(server_task);
    super::SERVER_REGISTRY.register_server("pager_server", srv.server_port());
    if let Some(name_server) =
        unsafe { (*core::ptr::addr_of!(super::name_server::NAME_SERVER)).as_ref() }
    {
        let _ = name_server.register("pager_server".into(), srv.server_port(), server_task);
    }
    unsafe {
        PAGER_SERVER = Some(srv);
    }
    crate::println!("Pager Server initialized");
}

pub fn pager_server() -> &'static PagerServer {
    unsafe {
        (*core::ptr::addr_of!(PAGER_SERVER))
            .as_ref()
            .expect("Pager server not initialized")
    }
}
