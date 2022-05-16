use std::env;
use std::process::Command;
use std::fs;
use std::collections::HashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use clap::Parser as ArgsParser;

#[derive(ArgsParser)]
struct Args {
    #[clap(long)]
    command: String,

    #[clap(long)]
    result_true: String,

    #[clap(long)]
    result_false: String,
}

#[derive(Serialize, Deserialize)]
struct ConsulService {
    Datacenter: String,
    ID: String,
    Node: String,
    Address: String,
    TaggedAddresses: HashMap<String, String>,
    NodeMeta: HashMap<String, String>,
    ServiceID: String,
    ServiceName: String,
    ServiceTags: Vec<String>,
    ServicePort: i32,
}

#[derive(Serialize, Deserialize)]
struct ConsulEntity {
    Datacenter: String,
    ID: String,
    Node: String,
    Address: String,
    TaggedAddresses: HashMap<String, String>,
    NodeMeta: HashMap<String, String>,
    Service: ConsulEntityService,
}

#[derive(Serialize, Deserialize)]
struct ConsulEntityService {
    ID: String,
    Service: String,
    Tags: Vec<String>,
    Port: i32,
}

/// Run cmd splitting the args by whitespace
fn run_cmd<'a>(cmd: &str) -> String {
    let whitespace_re = Regex::new(r"\s+").unwrap();
    let cmd_args: Vec<&str> = whitespace_re.split(&cmd).collect();
    let output = Command::new(cmd_args[0])
        .args(cmd_args[1..].to_vec())
        .output()
        .expect(&format!("failed to execute process '{}'", cmd));

    String::from_utf8(output.stdout).expect("process output invalid UTF-8")
}

fn main() {
    let args = Args::parse();

    let consul_data_dir = env::var("CONSUL_DATA_DIR").expect("Missing CONSUL_DATA_DIR");
    let consul_agent = env::var("CONSUL_AGENT").expect("Missing CONSUL_AGENT");

    let command_result = run_cmd(&args.command);
    let tags = if &command_result == &args.result_true {
        vec!["active"]
    } else if &command_result == &args.result_false {
        vec!["standby"]
    } else {
        vec![]
    };

    let node_id = fs::read_to_string(format!("{consul_data_dir}/node-id"))
        .expect("Failure reading node-id");

    let output_json = reqwest::blocking::get(
        format!(
            "http://{}/v1/catalog/service/mysql-orchestrator?{}",
            consul_agent,
            urlencoding::encode(&format!("filter=ID == \"{node_id}\"")),
        )
    )
        .unwrap().text().unwrap();

    let ConsulService {
        Datacenter,
        ID,
        Node,
        Address,
        TaggedAddresses,
        NodeMeta,
        ServiceID,
        ServiceName,
        mut ServiceTags,
        ServicePort,
    } = serde_json::from_str::<Vec<ConsulService>>(&output_json)
        .expect("service catalog JSON parse failure")
        .remove(0);

    for tag in tags {
        ServiceTags.push(tag.to_owned());
    }

    let entity = ConsulEntity {
        Datacenter,
        ID,
        Node,
        Address,
        TaggedAddresses,
        NodeMeta,
        Service: ConsulEntityService {
            ID: ServiceID,
            Service: ServiceName,
            Tags: ServiceTags,
            Port: ServicePort,
        },
    };

    let client = reqwest::blocking::Client::new();
    let final_response = client.put(
        format!("http://{consul_agent}/v1/catalog/register")
    )
        .body(serde_json::to_string(&entity).unwrap())
        .send().unwrap()
        .text().unwrap();

    println!("{}", final_response);
}
