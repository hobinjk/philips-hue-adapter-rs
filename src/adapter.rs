use nanomsg::{Protocol, Socket};
use serde_json::{self, Map, Value};
use std::io::{self, Read, Write};

const BASE_URL: &'static str = "ipc:///tmp";
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

#[derive(Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all="camelCase")]
enum GatewayPluginMessage {
    #[serde(rename_all = "camelCase")]
    RegisterPluginReply {
        plugin_id: String,
        ipc_base_addr: String,
    },
    #[serde(rename_all = "camelCase")]
    UnloadPlugin {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    UnloadAdapter {
        plugin_id: String,
        adapter_id: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all="camelCase")]
enum GatewayAdapterMessage {
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
#[serde(tag = "messageType", content = "data", rename_all="camelCase")]
enum PluginGatewayMessage {
    #[serde(rename_all = "camelCase")]
    RegisterPlugin {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    PluginUnloaded {
        plugin_id: String,
    },
    #[serde(rename_all = "camelCase")]
    AdapterUnloaded {
        plugin_id: String,
        adapter_id: String,
    },
}

#[derive(Serialize)]
#[serde(tag = "messageType", content = "data", rename_all="camelCase")]
enum AdapterGatewayMessage {
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
struct Property {
    name: String,
    value: f64,
}

pub fn register_plugin(id: &str) -> Result<(), io::Error> {
    let mut socket = Socket::new(Protocol::Req)?;
    let mut endpoint = socket.connect(ADAPTER_MANAGER_URL)?;
    let req = RegisterPlugin::new(id).json().to_string();
    socket.write_all(req.as_bytes())?;
    let mut rep = String::new();
    socket.read_to_string(&mut rep)?;
    println!("We got it! {}", rep);
    let msg: GatewayPluginMessage = serde_json::from_str(&rep)?;
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
        GatewayPluginMessage::RegisterPluginReply {ipc_base_addr, ..} => {
            ipc_base_addr
        },
        _ => {
            return Err(io::Error::new(io::ErrorKind::Other, format!("gateway sent unexpected message {}", rep)));
        }
    };

    let mut socket_pair = Socket::new(Protocol::Pair)?;
    let mut endpoint_pair = socket_pair.connect(&format!("{}/{}", BASE_URL, &ipc_base_addr))?;

    // wee woo wee woo
    for i in 1..10 {
        socket_pair.read_to_string(&mut rep)?;
        println!("we got another {}", rep);
    }

    endpoint.shutdown()?;
    endpoint_pair.shutdown()?;

    Ok(())
}
