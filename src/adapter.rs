use nanomsg::{Protocol, Socket, Endpoint};
use serde_json::{self, Map, Value};
use std::io::{self, Read, Write};

const BASE_URL: &'static str = "ipc:///tmp";
const ADAPTER_MANAGER_URL: &'static str = "ipc:///tmp/gateway.addonManager";

#[derive(Serialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
enum PluginRegisterMessage {
    #[serde(rename_all = "camelCase")]
    RegisterPlugin {
        plugin_id: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
enum GatewayRegisterMessage {
    #[serde(rename_all = "camelCase")]
    RegisterPluginReply {
        plugin_id: String,
        ipc_base_addr: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
pub enum GatewayMessage {
    #[serde(rename_all = "camelCase")]
    UnloadPlugin {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    UnloadAdapter {
        plugin_id: String,
        adapter_id: String,
    },

    #[serde(rename_all = "camelCase")]
    SetProperty {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
        property: Property,
    },
    #[serde(rename_all = "camelCase")]
    StartPairing {
        plugin_id: String,
        adapter_id: String,
        timeout: f64,
    },
    #[serde(rename_all = "camelCase")]
    CancelPairing {
        plugin_id: String,
        adapter_id: String,
    },
    #[serde(rename_all = "camelCase")]
    RemoveThing {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
    },
    #[serde(rename_all = "camelCase")]
    CancelRemoveThing {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
    },
}

#[derive(Serialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
enum PluginMessage {
    #[serde(rename_all = "camelCase")]
    PluginUnloaded {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    AdapterUnloaded {
        plugin_id: String,
        adapter_id: String,
    },

    #[serde(rename_all = "camelCase")]
    AddAdapter {
        plugin_id: String,
        adapter_id: String,
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    HandleDeviceAdded {
        plugin_id: String,
        adapter_id: String,
        id: String,
        name: String,
        typ: String,
        properties: Map<String, Value>,
        actions: Map<String, Value>,
    },
    #[serde(rename_all = "camelCase")]
    HandleDeviceRemoved {
        plugin_id: String,
        adapter_id: String,
        id: String,
    },
    #[serde(rename_all = "camelCase")]
    PropertyChanged {
        plugin_id: String,
        adapter_id: String,
        device_id: String,
        property: Property,
    },
}

#[derive(Deserialize, Serialize)]
pub struct Property {
    name: String,
    value: f64,
}

pub trait Handler {
    fn handle_msg(&self, msg: GatewayMessage) -> () {
    }
}

struct Plugin {
    socket: Socket,
    endpoint: Endpoint,
    handler: Box<Handler>,
}

impl Plugin {
    fn run(&mut self) {
        loop {
            let mut buf = Vec::new();
            match self.socket.nb_read_to_end(&mut buf) {
                Ok(_) => {},
                Err(_) => {
                    // sleep here?
                    continue;
                }
            };

            let req: GatewayMessage = match serde_json::from_slice(&buf) {
                Ok(req) => req,
                Err(e) => {
                    println!("Read junk {}", e);
                    continue;
                }
            };

            self.handler.handle_msg(req)
        }
    }

    fn send_msg(&mut self, msg: PluginMessage) -> Result<(), io::Error> {
        self.socket.write_all(serde_json::to_string(&msg)?.as_bytes())
    }
}

pub fn register_plugin(id: &str, plugin_handler: &Handler) -> Result<(), io::Error> {
    let mut socket = Socket::new(Protocol::Req)?;
    let mut endpoint = socket.connect(ADAPTER_MANAGER_URL)?;
    let req = PluginRegisterMessage::RegisterPlugin {
        plugin_id: id.to_string()
    };
    socket.write_all(serde_json::to_string(&req)?.as_bytes())?;
    let mut rep = String::new();
    socket.read_to_string(&mut rep)?;
    endpoint.shutdown()?;
    println!("We got it! {}", rep);
    let msg: GatewayRegisterMessage = serde_json::from_str(&rep)?;
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

    let ipc_base_addr = match msg {
        GatewayRegisterMessage::RegisterPluginReply {ipc_base_addr, ..} => {
            ipc_base_addr
        },
    };

    let mut socket_pair = Socket::new(Protocol::Pair)?;
    let mut endpoint_pair = socket_pair.connect(&format!("{}/{}", BASE_URL, &ipc_base_addr))?;

    // wee woo wee woo
    for i in 1..10 {
        socket_pair.read_to_string(&mut rep)?;
        let msg: GatewayMessage = serde_json::from_str(&rep)?;
        plugin_handler.handle_msg(msg);
    }

    endpoint_pair.shutdown()?;

    Ok(())
}
