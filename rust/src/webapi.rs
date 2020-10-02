use filecoin_webapi::polling::PollingState;
use log::*;
use rand::seq::SliceRandom;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{from_value, json, Value};
use std::fs::{self};
use std::thread;
use std::time::Duration;

lazy_static! {
    static ref REQWEST_CLIENT: Client = Client::new();
    static ref CONFIG: WebApiConfig = {
        let f = fs::File::open("/etc/filecoin-webapi.yaml").unwrap();
        let config = serde_yaml::from_reader(f).unwrap();

        info!("filecoin-webapi config: {:?}", config);
        config
    };
}

#[derive(Deserialize, Serialize, Debug)]
struct WebApiConfig {
    url: String,
    servers: Vec<String>,
}

impl WebApiConfig {
    fn pick_server(&self) -> &String {
        self.servers
            .choose(&mut rand::thread_rng())
            .expect("No server found!")
    }
}

/*=== webapi macros ===*/

// #[allow(dead_code)]
// pub(crate) fn webapi_upload<F: AsRef<str>>(file: F) -> Result<String, String> {
//     let mut f = File::open(file.as_ref()).map_err(|e| format!("{:?}", e))?;
//     let mut buf = vec![];
//     f.read_to_end(&mut buf).map_err(|e| format!("{:?}", e))?;
//
//     let form = Form::new()
//         .file("webapi_upload", file.as_ref())
//         .map_err(|e| format!("{:?}", e))?;
//     let post = REQWEST_CLIENT.post(&format!("{}/sys/upload_file", CONFIG.url));
//     let response = post
//         .multipart(form)
//         .send()
//         .map_err(|e| format!("{:?}", e))?
//         .text()
//         .map_err(|e| format!("{:?}", e))?;
//     let upload_file: Option<String> =
//         serde_json::from_str(&response).map_err(|e| format!("{:?}", e))?;
//
//     upload_file.ok_or("None".to_string())
// }

#[derive(Debug)]
enum WebApiError {
    StatusError(u16),
    Error(String),
}

/// pick server to post, if successful, return value and server host
/// path: request resource path
/// json: request data
#[allow(dead_code)]
fn webapi_post_pick<T: Serialize + ?Sized>(
    path: &str,
    json: &T,
) -> Result<(String, Value), String> {
    loop {
        let server = CONFIG.pick_server();
        let url = format!("{}{}", server, path);
        match webapi_post(&url, json) {
            Ok(val) => return Ok((server.clone(), val)),
            Err(WebApiError::Error(err)) => return Err(err),
            Err(WebApiError::StatusError(stat)) => {
                // TooManyRequests
                if stat != 429 {
                    return Err(format!("Err with code: {}", stat));
                }
            }
        }

        // sleep
        debug!("TooManyRequests in server {}, waiting...", server);
        thread::sleep(Duration::from_secs(60));
    }
}

#[allow(dead_code)]
fn webapi_post<T: Serialize + ?Sized>(url: &str, json: &T) -> Result<Value, WebApiError> {
    trace!("webapi_post url: {}", url);

    let post = REQWEST_CLIENT.post(url);
    let text = match post.json(json).send() {
        Ok(response) => {
            let stat = response.status().as_u16();
            if stat != 200 {
                return Err(WebApiError::StatusError(stat));
            }

            response
                .text()
                .map_err(|e| WebApiError::Error(format!("{:?}", e)))?
        }
        Err(e) => return Err(WebApiError::Error(format!("{:?}", e))),
    };

    let value: Value =
        serde_json::from_str(&text).map_err(|e| WebApiError::Error(format!("{:?}", e)))?;
    if value.get("Err").is_some() {
        return Err(WebApiError::Error(format!("{:?}", value)));
    }

    return Ok(value);
}

#[allow(dead_code)]
pub(crate) fn webapi_post_polling<T: Serialize + ?Sized>(
    path: &str,
    json: &T,
) -> Result<Value, String> {
    let (server, state) = match webapi_post_pick(path, json) {
        Ok((server, value)) => {
            let state: PollingState = from_value(value).map_err(|e| format!("{:?}", e))?;
            (server, state)
        }
        Err(e) => return Err(e),
    };

    info!(
        "webapi_post_polling request server: {}, state: {:?}",
        server, state
    );

    let proc_id = match state {
        PollingState::Started(val) => val,
        e @ _ => {
            return Err(format!("webapi_post_polling response error: {:?}", e));
        }
    };

    loop {
        let url = format!("{}{}", server, "sys/query_state");
        let val = webapi_post(&url, &json!(proc_id)).map_err(|e| format!("{:?}", e))?;
        let poll_state: PollingState = from_value(val).map_err(|e| format!("{:?}", e))?;

        match poll_state {
            PollingState::Done(result) => return Ok(result),
            PollingState::Pending => {
                trace!("proc_id: {}, Pending...", proc_id);
            }
            e @ _ => {
                debug!("Polling Error: {:?}", e);
                return Err(format!("poll_state error: {:?}", e));
            }
        }

        // sleep 30s
        let time = Duration::from_secs(30);
        thread::sleep(time);
    }
}

// #[allow(unused_macros)]
// macro_rules! webapi_post {
//     ($path:literal, $json:expr) => {
//         crate::util::rpc::webapi_post($path, $json);
//     };
// }

#[allow(unused_macros)]
macro_rules! webapi_post_polling {
    ($path:literal, $json:expr) => {
        crate::util::rpc::webapi_post_polling($path, $json);
    };
}
