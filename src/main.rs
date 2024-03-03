use std::{collections::HashMap, env};

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use getopts::Options;

use swayipc::{Connection, Error, Event, EventType, Node, WindowChange};

#[derive(Debug)]
struct LayoutState {
    comm_conn: Connection,
    default_lang: Option<String>,
    prev_id: Option<String>,
    state: HashMap<String, HashMap<String, i32>>,
    tabbed: Vec<String>,
}

impl LayoutState {
    fn on_focus(&mut self, key: &str) {
        if let Some(key) = self.prev_id.clone() {
            let layoutmap = self._get_lang();
            self.state.insert(key, layoutmap);
        }

        self._set_lang(&key);
        self.prev_id = Some(key.to_string());
    }

    fn on_close(&mut self, key: &str) {
        info!("Closed window: {}", key);
        self.state.remove(key);
        if self.prev_id == Some(key.to_string()) {
            self.prev_id = None;
        }
    }

    fn _set_lang(&mut self, key: &str) {
        if let Some(map) = self.state.get(key) {
            for (input_id, lo_idx) in map {
                let _ = self
                    .comm_conn
                    .run_command(format!("input {input_id} xkb_switch_layout {lo_idx}"));
            }
        } else {
            if let Some(lang) = &self.default_lang {
                for input in self.comm_conn.get_inputs().unwrap() {
                    for (lo_idx, lo_name) in input.xkb_layout_names.iter().enumerate() {
                        if lo_name == lang {
                            let _ = self.comm_conn.run_command(format!(
                                "input {} xkb_switch_layout {lo_idx}",
                                input.identifier
                            ));
                        }
                    }
                }
            }
        }
    }

    fn _get_lang(&mut self) -> HashMap<String, i32> {
        let mut input_map: HashMap<String, i32> = HashMap::new();
        for input in self.comm_conn.get_inputs().unwrap() {
            if input.input_type != "keyboard" {
                continue;
            }
            input_map.insert(
                input.identifier,
                input
                    .xkb_active_layout_index
                    .expect("Input will always have active layout because it is keyboard"),
            );
        }
        input_map
    }

    fn make_map_key(&self, container: Node) -> String {
        let mut key = container.id.to_string();
        if let Some(app_id) = container.app_id {
            if self.tabbed.contains(&app_id) {
                if let Some(name) = container.name {
                    key.push_str(&name)
                }
            }
        }
        key
    }
}

fn event_loop(state: &mut LayoutState) -> Result<(), Error> {
    let event_conn = Connection::new()?;
    info!("Started event connection to sway-ipc: {:?}", event_conn);
    let mut events = event_conn.subscribe([EventType::Window])?;
    while let Some(event) = events.next() {
        if let Event::Window(w) = event.unwrap() {
            info!("Got an event: {:?}", w);
            match w.change {
                WindowChange::Focus | WindowChange::Title => {
                    state.on_focus(&state.make_map_key(w.container))
                }
                WindowChange::Close => state.on_close(&state.make_map_key(w.container)),
                _ => continue,
            }
        }
    }
    Ok(())
}

fn start(default_lang: Option<String>, tabbed: Vec<String>) {
    let mut state = LayoutState {
        comm_conn: Connection::new().unwrap(),
        default_lang,
        state: HashMap::new(),
        prev_id: None,
        tabbed,
    };
    info!("State: {:?}", state);
    info!("Entering main event loop.");

    if let Err(err) = event_loop(&mut state) {
        panic!("Error while polling sway events: {:?}", err);
    }

    info!("Main event loop finished.");
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    pretty_env_logger::init();

    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    info!("Arguments: {:?}", args);

    let mut opts = Options::new();
    opts.optopt(
        "D",
        "default-lang",
        "Set default language to use. Check man sway-ipc for more info on <xkb_layout_name>.",
        "<xkb_layout_name>",
    );
    opts.optopt("T", "tabbed-apps", "Set tabbed apps list.", "[app_ids ...]");
    opts.optflag("h", "help", "Print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            error!("Error parsing opts {}", f.to_string());
            std::process::exit(1)
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }

    let default_lang = matches.opt_str("default-lang");
    info!("default-lang: {:?}", &default_lang);

    let mut tabbed_apps: Vec<String> = vec![];
    if let Some(apps) = matches.opt_str("tabbed-apps") {
        for app in apps.split(",") {
            tabbed_apps.push(app.to_string())
        }
    }
    info!("tabbed-apps: {:?}", tabbed_apps);

    start(default_lang, tabbed_apps);
}
