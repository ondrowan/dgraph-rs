use chrono::prelude::*;
use dgraph::{Dgraph, make_dgraph};
use serde_derive::{Serialize, Deserialize};
use slog::{Drain, slog_info, slog_o};
use slog_scope::{info};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Root {
	pub me: Vec<Person>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct School {
	pub name: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Location {
	#[serde(rename = "type")]
	pub kind: String,
	pub coordinates: Vec<f64>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Person {
	pub name: String,
	pub age: Option<u8>,
	pub dob: Option<DateTime<Utc>>,
	pub married: Option<bool>,
	#[serde(rename = "raw_bytes")]
	pub raw: Option<Vec<u8>>,
	#[serde(rename = "friend")]
	pub friends: Option<Vec<Person>>,
	#[serde(rename = "loc")]
	pub location: Option<Location>,
	pub school: Option<Vec<School>>,
}

fn drop_all(client: &Dgraph) {
    let op_cleanup = dgraph::Operation {
        drop_all: true,
        ..Default::default()
    };

    client.alter(&op_cleanup).expect("drop schema");
}

fn set_schema(client: &Dgraph) {
    let op_schema = dgraph::Operation {
        schema: r#"
            name: string @index(exact) .
            age: int .
            married: bool .
            loc: geo .
            dob: datetime .
        "#.to_string(),
        ..Default::default()
    };

    client.alter(&op_schema).expect("set schema");
}

fn create_data(client: &Dgraph) {
    let mut txn = client.new_txn();

    let dob = Utc.ymd(1980, 1, 1).and_hms(23, 0, 0);
    // While setting an object if a struct has a Uid then its properties in the graph are updated
    // else a new node is created.
    // In the example below new nodes for Alice, Bob and Charlie and school are created (since they
    // dont have a Uid).
    let p = Person {
        name: "Alice".to_string(),
        age: Some(26),
        married: Some(true),
        location: Some(Location {
            kind: "Point".to_string(),
            coordinates: vec![1.1f64, 2f64],
        }),
        dob: Some(dob),
        //raw: "raw_bytes".as_bytes().to_vec(),
        friends: Some(vec![
            Person {
                name: "Bob".to_string(),
                age: Some(24),
                ..Default::default()
            },
            Person {
                name: "Charlie".to_string(),
                age: Some(29),
                ..Default::default()
            },
        ]),
        school: Some(vec![
            School {
                name: "Crown Public School".to_string(),
            },
        ]),
        ..Default::default()
    };

    // Run mutation
    let mut mutation = dgraph::Mutation::new(); 
    mutation.set_set_json(serde_json::to_vec(&p).expect("invalid json"));
    let assigned = txn.mutate(mutation).expect("failed to create data");

    // Commit transaction
    txn.commit().expect("Fail to commit mutation");

    // Get uid of the outermost object (person named "Alice").
    // Assigned#getUidsMap() returns a map from blank node names to uids.
    // For a json mutation, blank node names "blank-0", "blank-1", ... are used
    // for all the created nodes.
    info!("Created person named 'Alice' with uid = {}", assigned.uids["blank-0"]);

    info!("All created nodes (map from blank node names to uids):");
    for (key, val) in assigned.uids.iter() {
        info!("\t{} => {}", key, val);
    }
}

fn query_data(client: &Dgraph) {
    let query = r#"query all($a: string){
        me(func: eq(name, $a)) {
            name
            dob
            age
            loc
            raw_bytes
            married
            friend {
                name
                age
            }
            school {
                name
            }
        }
    }"#.to_string();

    let mut vars = HashMap::new();
    vars.insert("$a".to_string(), "Alice".to_string());

    let resp = client.new_readonly_txn().query_with_vars(query, vars).expect("query");
    let root: Root = serde_json::from_slice(&resp.json).expect("parsing");
    info!("Root: {:#?}", root);
}

fn run_example() {
    info!("connect to dgraph via grpc at localhost:9080");

    let client = make_dgraph!(dgraph::new_dgraph_client("localhost:9080"));

    info!("dropping all schema");
    drop_all(&client);

    info!("setup schema");
    set_schema(&client);

    info!("push data");
    create_data(&client);

    info!("query");
    query_data(&client);
}

fn main() {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let log = slog::Logger::root(
        slog_term::FullFormat::new(plain)
        .build().fuse(), slog_o!()
    );

    // Make sure to save the guard, see documentation for more information
    let _guard = slog_scope::set_global_logger(log);
    slog_scope::scope(&slog_scope::logger().new(slog_o!("scope" => "1")), run_example);
}
