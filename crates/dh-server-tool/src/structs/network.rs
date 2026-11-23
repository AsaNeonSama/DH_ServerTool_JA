#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicIpResult {
    pub ip: Option<String>,
    pub error: Option<String>,
}
