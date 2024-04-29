pub(crate) const UNKNOWN_CONTAINER_LABEL: &'static str = "UNKNOWN";

#[allow(dead_code)]
pub struct ExecutionReport {
    pub name: String,
    pub command: String,
    pub retval: u32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
