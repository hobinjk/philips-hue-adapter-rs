extern crate nanomsg;
extern crate reqwest;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use std::io;
use reqwest::Client;
mod adapter;
use adapter::{register_plugin, Handler, GatewayMessage};

#[derive(Deserialize)]
struct DiscoveredBridge {
    id: String,
    internalipaddress: String
}

#[derive(Serialize, Deserialize)]
struct LightProperties {
    on: bool,
    hue: i32,
    sat: i32,
    bri: i32
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

enum PairState {
    Unpaired,
    Pairing,
    Paired(String)
}

struct Bridge  {
    id: String,
    ip: String,
    state: PairState,
}

impl Bridge {
    fn new(id: &str, ip: &str) -> Bridge {
        Bridge {
            id: id.to_string(),
            ip: ip.to_string(),
            state: PairState::Unpaired,
        }
    }

    fn start_pairing(&mut self) {
        self.state = PairState::Pairing;
        // send a request every 500 ms to the API
        // somehow make this cancellable
    }

    fn cancel_pairing(&mut self) {
        // cancel this thread?
    }

    fn to_adapter(&self) -> Option<Adapter> {
        if let PairState::Paired(ref username) = self.state {
            Some(Adapter {
                id: self.id.clone(),
                ip: self.ip.clone(),
                username: username.clone()
            })
        } else {
            None
        }
    }
}

struct Adapter {
    id: String,
    ip: String,
    username: String,
}

impl Adapter {
    fn api_uri(&self) -> String {
        format!("http://{}/api/{}/", self.ip, self.username)
    }

    fn send_properties(&self, light_id: &str, props: LightProperties) -> Result<(), io::Error>{
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
}

struct PhilipsHuePlugin {
    adapter: Adapter,
}

impl Handler for PhilipsHuePlugin {
    fn handle_msg(&self, msg: GatewayMessage) -> () {
        let light_id = "1";

        let props = LightProperties {
            on: true,
            hue: 0,
            sat: 0,
            bri: 255
        };

        self.adapter.send_properties(&light_id, props);
    }
}

fn main() {
    println!("hello world!");
    let bridges = discover_bridges().unwrap();
    let adapter = bridges[0].to_adapter().unwrap();
    let plugin = PhilipsHuePlugin { adapter };
    register_plugin("philips-hue", &plugin).unwrap();
    // spawn::something(|| {
    //     let bridges = discover_bridges().unwrap();
    //     // pair bridges then put adapters on a channel
    // });

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
