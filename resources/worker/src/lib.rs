use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub id: i64,
    pub inputs: Vec<Vec<u8>>,
    pub code: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub id: i64,
    pub outputs: Vec<Result<i64, String>>,
    pub times: Vec<u64>,
}
