extern crate nanomsg;
extern crate reqwest;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::Ipv4Addr;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use nanomsg::{Protocol, Socket};
use reqwest::Client;
use serde_json::{Map, Value};

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

#[derive(Debug, Deserialize)]
#[serde(tag = "messageType", content = "data", rename_all = "camelCase")]
enum GatewayMessage {
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

#[derive(Debug, Deserialize, Serialize)]
struct Property {
    name: String,
    value: Value,
}

struct GatewayBridge {
    id: String,
    msg_sender: Sender<GatewayMessage>,
    msg_receiver: Receiver<PluginMessage>
}

impl GatewayBridge {
    fn new(id: &str) -> (GatewayBridge, Sender<PluginMessage>, Receiver<GatewayMessage>) {
        let (gp_sender, gp_receiver) = channel();
        let (pg_sender, pg_receiver) = channel();
        (
            GatewayBridge {
                id: id.to_string(),
                msg_sender: gp_sender,
                msg_receiver: pg_receiver,
            },
            pg_sender,
            gp_receiver
        )
    }

    fn run_forever(&mut self) -> Result<(), io::Error> {
        let mut socket = Socket::new(Protocol::Req)?;
        let mut endpoint = socket.connect(ADAPTER_MANAGER_URL)?;
        let req = PluginRegisterMessage::RegisterPlugin {
            plugin_id: self.id.to_string()
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

        let mut buf = Vec::new();

        loop {
            let read_status = socket.nb_read_to_end(&mut buf);
            if read_status.is_ok() {
                match serde_json::from_slice(&buf) {
                    Ok(msg) => {
                        self.msg_sender.send(msg).unwrap();
                    },
                    _ => {
                    }
                }
            }

            if let Ok(msg_to_send) = self.msg_receiver.try_recv() {
                socket_pair.write_all(serde_json::to_string(&msg_to_send)?.as_bytes()).unwrap();
                match msg_to_send {
                    PluginMessage::PluginUnloaded {..} => {
                        println!("run_forever exiting");
                        endpoint_pair.shutdown()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            thread::sleep(Duration::from_millis(33));
        }
    }
}

#[derive(Deserialize)]
struct DiscoveredBridge {
    id: String,
    internalipaddress: String
}

#[derive(Serialize, Deserialize)]
struct LightProperties {
    on: bool,
    hue: f64,
    sat: f64,
    bri: f64,
}

fn to_io_error<E>(err: E) -> io::Error
    where E: Into<Box<std::error::Error+Send+Sync>> {
    io::Error::new(io::ErrorKind::Other, err)
}

fn discover_bridges() -> Result<Vec<Bridge>, io::Error> {
    let uri = "https://www.meethue.com/api/nupnp";
    let client = Client::new();
    let mut res = client.get(uri).send().map_err(to_io_error)?;

    let bridges: Vec<DiscoveredBridge> = res.json().map_err(to_io_error)?;

    Ok(bridges.into_iter().map(|bridge| {
        Bridge::new(&bridge.id, &bridge.internalipaddress)
    }).collect())
}

#[derive(PartialEq)]
enum PairState {
    Unpaired,
    Pairing,
    Paired(String)
}

struct Bridge  {
    id: String,
    ip: String,
    state: Arc<Mutex<PairState>>,
}

impl Bridge {
    fn new(id: &str, ip: &str) -> Bridge {
        Bridge {
            id: id.to_string(),
            ip: ip.to_string(),
            state: Arc::new(Mutex::new(PairState::Unpaired)),
        }
    }

    fn start_pairing(&self) {
        match *self.state.lock().unwrap() {
            PairState::Unpaired => {
            },
            _ => {
                println!("Already pairing or paired");
                return;
            }
        }

        let state = Arc::clone(&self.state);
        let uri = format!("http://{}/api", self.ip);

        // send a request every 500 ms to the API for 50 seconds
        thread::spawn(move || {
            for _ in 0..100 {
                if *state.lock().unwrap() != PairState::Pairing {
                    return;
                }
                println!("happening");

                let client = Client::new();
                let mut res = client.put(&uri)
                    .json(&json!({
                        "devicetype": "mozilla_gateway#PhilipsHueAdapter"
                    }))
                    .send().unwrap();

                if !res.status().is_success() {
                    println!("Unsuccessful response: {:?}", res);
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                let v: Value = res.json().unwrap();
                if v["error"] != Value::Null || v["success"] == Value::Null {
                    println!("Unsuccessful value: {:?}", v);
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                if let Value::String(ref username) = v["success"]["username"] {
                    *state.lock().unwrap() = PairState::Paired(username.clone());
                    return;
                }
                thread::sleep(Duration::from_millis(500));
            }
        });
    }

    fn cancel_pairing(&self) {
        let mut state = self.state.lock().unwrap();
        if *state == PairState::Pairing {
            *state = PairState::Unpaired
        }
    }

    fn to_adapter(&self) -> Option<Adapter> {
        let addr: Ipv4Addr = match self.ip.parse() {
            Ok(addr) => addr,
            Err(_) => {
                return None;
            }
        };

        if let PairState::Paired(ref username) = *self.state.lock().unwrap() {
            Some(Adapter {
                id: self.id.clone(),
                ip: addr,
                username: username.clone(),
                devices: HashMap::new(),
            })
        } else {
            None
        }
    }
}

type LightId = String;

struct Device {
    id: String,
    light_id: LightId,
    props: LightProperties,
}

impl Device {
    fn new(bridge_id: &str, light_id: LightId, props: LightProperties) -> Device {
        Device {
            id: format!("philips-hue-{}-{}", bridge_id, light_id),
            light_id: light_id,
            props: props,
        }
    }
}

struct Adapter {
    id: String,
    ip: Ipv4Addr,
    username: String,
    devices: HashMap<LightId, Device>
}

impl Adapter {
    fn api_uri(&self) -> String {
        format!("http://{}/api/{}/", self.ip, self.username)
    }

    fn put_props(&self, light_id: LightId, props: LightProperties) -> Result<(), io::Error>{
        let uri = format!("{}/lights/{}/state", self.api_uri(), light_id);
        let client = reqwest::Client::new();
        let res = client.put(&uri)
            .json(&props)
            .send().map_err(to_io_error)?;

        if res.status().is_success() {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "response not okay"))
        }
    }

    fn get_props(&self, light_id: LightId) -> Result<LightProperties, io::Error>{
        let uri = format!("{}/lights/{}/state", self.api_uri(), light_id);
        let client = reqwest::Client::new();
        let mut res = client.get(&uri)
            .send().map_err(to_io_error)?;

        let props: LightProperties = res.json().map_err(to_io_error)?;

        Ok(props)
    }
}

struct Plugin {
    unpaired_bridges: Vec<Bridge>,
    adapters: HashMap<String, Adapter>,
    sender: Sender<PluginMessage>,
    receiver: Receiver<GatewayMessage>,
}

impl Plugin {
    fn new(sender: Sender<PluginMessage>, receiver: Receiver<GatewayMessage>) -> Plugin {
        Plugin {
            unpaired_bridges: Vec::new(),
            adapters: HashMap::new(),
            sender: sender,
            receiver: receiver,
        }
    }

    fn handle_msg(&self, msg: GatewayMessage) -> Result<(), io::Error> {
        match msg {
            GatewayMessage::SetProperty {
                plugin_id: _,
                adapter_id,
                device_id,
                property
            } => {
                let adapter = &self.adapters[&adapter_id];
                println!("uhhh: {:?}", property);

                let old_props = &adapter.devices[&device_id].props;

                let mut props = LightProperties {
                    on: old_props.on,
                    hue: old_props.hue,
                    sat: old_props.sat,
                    bri: old_props.bri,
                };

                match (property.name.as_ref(), property.value) {
                    ("on", Value::Bool(on)) => {
                        props.on = on;
                    },
                    ("hue", Value::Number(hue)) => {
                        props.hue = hue.as_f64().unwrap();
                    },
                    ("saturation", Value::Number(saturation)) => {
                        props.sat = saturation.as_f64().unwrap();
                    },
                    ("brightness", Value::Number(brightness)) => {
                        props.bri = brightness.as_f64().unwrap();
                    },
                    _ => {
                        println!("Unknown property: {}", property.name);
                        return Ok(())
                    }
                };
                adapter.put_props(device_id, props)?;
            },
            GatewayMessage::UnloadPlugin {..} => {
            },
            GatewayMessage::UnloadAdapter {..} => {
            },
            GatewayMessage::StartPairing {
                plugin_id: _,
                adapter_id: _,
                timeout: _,
            } => {
                for bridge in &self.unpaired_bridges {
                    bridge.start_pairing();
                }
            },
            GatewayMessage::CancelPairing {
                plugin_id: _,
                adapter_id: _,
            } => {
                for bridge in &self.unpaired_bridges {
                    bridge.cancel_pairing();
                }
            },
            GatewayMessage::RemoveThing { .. } => {
            },
            GatewayMessage::CancelRemoveThing { .. } => {
            }
        }
        Ok(())
    }

    fn run_forever(&mut self) -> Result<(), io::Error> {
        self.unpaired_bridges = discover_bridges().unwrap();

        loop {
            match self.receiver.try_recv() {
                Ok(msg) => {
                    println!("recv: {:?}", msg);
                    self.handle_msg(msg)?;
                },
                _ => {}
            }
        }
    }
}

fn main() {
    let (mut gateway_bridge, msg_sender, msg_receiver) = GatewayBridge::new("philips-hue");
    thread::spawn(move || {
        gateway_bridge.run_forever().unwrap();
    });
    let mut plugin = Plugin::new(msg_sender, msg_receiver);
    plugin.run_forever().unwrap();

    // let adapters = map from id to adapter
    // select (nanomsg, paired bridges channel)
    // send a start/cancel pairing to the bridge proc if requested
    // dispatch commands to the addapters list
    // let light_id = "1";

    // let props = LightProperties {
    //     on: true,
    //     hue: 0,
    //     sat: 0,
    //     bri: 255
    // };
    // let _ = adapters[0].send_properties(light_id, props).unwrap();
}
