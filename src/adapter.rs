use nanomsg::{Protocol, Socket};
use serde_json;
use std::io::{self, Read, Write};

const ADAPTER_MANAGER_URL: &'static str = "ipc:///tmp/gateway.addonManager";

struct RegisterPlugin {
    id: String,
}

impl RegisterPlugin {
    fn new(id: &str) -> RegisterPlugin {
        RegisterPlugin {
            id: id.to_string()
        }
    }

    fn json(&self) -> serde_json::Value {
        json!({
            "messageType": "registerPlugin",
            "data": {
                "pluginId": self.id
            }
        })
    }
}

pub fn register_plugin(id: &str) -> Result<(), io::Error> {
    let mut socket = Socket::new(Protocol::Req)?;
    let mut endpoint = socket.connect(ADAPTER_MANAGER_URL)?;
    let req = RegisterPlugin::new(id).json().to_string();
    socket.write_all(req.as_bytes())?;
    let mut rep = String::new();
    socket.read_to_string(&mut rep)?;
    println!("We got it! {}", rep);
    // open a Req channel to adapterManager
    // send {messageType: 'registerPlugin', data: { pluginId: id }}
    // receives
    // {
    //  messageType: 'registerPluginReply',
    //  data: {
    //    pluginId: 'pluginId-string',
    //    ipcBaseAddr: 'gateway.plugin.xxx',
    //  },
    //}
    // connect to ipcBaseAddr as pair
    // then handle everything

    endpoint.shutdown()?;
    Ok(())
}
