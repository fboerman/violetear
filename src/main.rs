#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

extern crate yaml_rust;
extern crate zmq;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::io::BufReader;
use std::collections::HashMap;

use yaml_rust::{YamlLoader, yaml};
use std::{thread, time};

// enum to keep track in what state the parser is
#[derive(PartialEq)]
enum section {
    scanning,
    layout,
    initial,
}

// an edge in the dataflow diagram
#[derive(Default, Serialize, Deserialize)]
struct edge {
    from: String,
    to: String,
    tokensin: u32,
    tokensout: u32,
    currentholding: u32,
}

// a node in the dataflow diagram
// firing is the ammount of ticks that the node will be left firing
// time is the ammount of ticks a firing costs
#[derive(Default, Serialize, Deserialize)]
struct node {
    name: String,
    firing: u32,
    time: u32,
}

// scans a given string until a delimeter is reached, returns the index after the delimter and the produced string
fn scanUntilDelimeter(line: &String, delimeter: char, startindex: u32) -> (u32, String) {
    let mut s = String::new();
    let chars: Vec<_> = line.trim().chars().collect();
    let mut endindex = startindex;
    for i in startindex..chars.len() as u32 {
        if chars[i as usize] == delimeter {
            endindex = i + 1;
            break;
        }
        s.push(chars[i as usize]);
    }

    (endindex, s)
}

// tries to find an edge with given properties
fn findEdge<'a>(edges: &'a mut Vec<edge>, from: &String, to: &String) -> Option<&'a mut edge> {
    for e in edges {
        if e.from == *from && e.to == *to {
            return Some(e);
        }
    }

    None
}

// checks if all input edges of a node are satisfied in terms of tokens. returns true if node can fire, false otherwise
fn checkNodeInputs(edges: &Vec<edge>, n: &node) -> bool {
    if n.firing > 0 {
        //if node is still firing than it can not be fired again
        return false;
    }
    for e in edges {
        if e.to == n.name {
            //edge to our node so check if this edge has enough tokens to sustain firing node
            //if not than the node cannot fire
            if e.tokensout > e.currentholding {
                return false;
            }
        }
    }

    //all edges to this node have enough tokens so node can fire
    true
}

// trigger firing of given node, it updates all its input edges
fn fireNode(edges: &mut Vec<edge>, n: &mut node) {
    if n.firing > 0 {
        //if node is already firing dont do anything
        return;
    }
    for e in edges {
        // if e.from == n.name {
        //     //found edge leaving the node, add tokens
        //     e.currentholding += e.tokensin;
        // }
        if e.to == n.name {
            //foudn edge entering node, subtract tokens
            e.currentholding -= e.tokensout;
        }
    }
    n.firing = n.time;
}

fn tickNodes(nodes: &mut HashMap<&str, node>, edges: &mut Vec<edge>, timestep: u32) {
    for (_, n) in nodes {
        if n.firing > 0 {
            //if node is in firingmode, decrease its firetime
            n.firing -= timestep;
            if n.firing == 0 {
                //node is done with firing, update output edges
                for e in edges.iter_mut() {
                    if e.from == n.name {
                        //output node found, update its tokens
                        e.currentholding += e.tokensin;
                    }
                }
            }
        }
    }
}

fn main() {

    //first load and parse the nodes from nodes.yaml config file
    //use the yaml crate for it
    let yamlconfigpath = Path::new("nodes.yaml");
    let mut file1 = match File::open(&yamlconfigpath) {
        Err(why) => {
            panic!("Couldn't open nodes.yaml config file: {}",
                   why.description())
        }
        Ok(file) => file,
    };
    let mut yamlcontent = String::new();
    match file1.read_to_string(&mut yamlcontent) {
        Err(_) => panic!("Couldnt read nodes.yaml"),
        Ok(_) => file1,
    };

    let yamls = YamlLoader::load_from_str(yamlcontent.as_str()).unwrap();
    let yamlnodes = &yamls[0];

    //holding all the objects
    let mut edges: Vec<edge> = Vec::new();
    let mut nodes: HashMap<&str, node> = HashMap::new();

    //iterate through the hashmap in the yamlfile and create node objects
    match *yamlnodes {
        yaml::Yaml::Hash(ref h) => {
            for (nme, settings) in h {
                let n = node {
                    name: nme.as_str().unwrap().to_string().clone(),
                    time: settings["time"].as_i64().unwrap() as u32, //yaml library doesnt support immideate conversion to u32
                    firing: 0,
                };
                nodes.insert(nme.as_str().unwrap(), n);
            }
        }
        _ => panic!("Parser error: wrong yamltype"),
    };




    //now load and parse the layout from the layout.df file
    let layoutconfigpath = Path::new("layout.df");
    let file2 = match File::open(&layoutconfigpath) {
        Err(why) => panic!("Couldn't open layout.df config file: {}", why.description()),
        Ok(file) => file,        
    };
    //wrap reader in buffer
    let bufread = BufReader::new(file2);
    let mut status = section::scanning;
    for line in bufread.lines().filter_map(|result| result.ok()) {
        //scan for header
        match line.trim() {
            "[layout]" => {
                status = section::layout;
                continue;
            }
            "[initial]" => {
                status = section::initial;
                continue;
            }
            _ => {}
        }
        if status == section::scanning {
            //we have not yet begun on a section, keep looking
            continue;
        }

        match status {
            section::layout => {
                //we are in the layout section
                //parse the following layout: <nodename>:<intokens>-><outtokens>:<nodename>
                //and push it in the edges vector
                let mut E = edge { ..Default::default() };
                let fromtuple = scanUntilDelimeter(&line, ':', 0);
                E.from = fromtuple.1;
                let intokentuple = scanUntilDelimeter(&line, '-', fromtuple.0);
                E.tokensin = match intokentuple.1.parse() {
                    Ok(num) => num,
                    Err(_) => panic!("Parser error: non integer value for tokensize"),
                };
                let outtokentuple = scanUntilDelimeter(&line, ':', intokentuple.0 + 1);
                E.tokensout = match outtokentuple.1.parse() {
                    Ok(num) => num,
                    Err(_) => panic!("Parser error: non integer value for tokensize"),
                };
                let totuple = scanUntilDelimeter(&line, ':', outtokentuple.0);
                E.to = totuple.1;
                E.currentholding = 0;
                edges.push(E);
            }
            section::initial => {
                //we are in the initial section
                //parse the follow format: <nodename>-><nodename>:initialtokens
                //create the node objects and put them in the hashmap by name
                let mut tuple = scanUntilDelimeter(&line, '-', 0);
                let from = tuple.1;
                tuple = scanUntilDelimeter(&line, ':', tuple.0 + 1);
                let to = tuple.1;
                let mut edg = match findEdge(&mut edges, &from, &to) {
                    Some(e) => e,
                    None => panic!("Parser error: non existing edge in  initial"),
                };
                tuple = scanUntilDelimeter(&line, ':', tuple.0);
                edg.currentholding = match tuple.1.parse() {
                    Ok(num) => num,
                    Err(_) => panic!("Parser error: non integer value for initial tokensize"),
                };

            }
            _ => {
                continue;
            }
        }
    }

    println!("Parsed {} edges and {} nodes", edges.len(), nodes.len());

    //parsing done
    //start a zmq server to publish events to
    let context = zmq::Context::new();
    let publisher = context.socket(zmq::PUB).unwrap();
    assert!(publisher.bind("tcp://127.0.0.1:5556").is_ok());

    //now iterate through all nodes, check if condition for firing are met
    //than fire them all
    let second = time::Duration::from_secs(1);
    let mut i = 0;
    loop {
        println!("{}", i);
        //tick all firing nodes
        tickNodes(&mut nodes, &mut edges, 1);

        //find all nodes that can currently fire
        let mut firingnodes: Vec<String> = Vec::new();
        for (_, n) in &nodes {
            if checkNodeInputs(&edges, &n) {
                firingnodes.push(n.name.clone());
            }
        }

        //fire all nodes found in the previous step
        //this two step system is necesarry to prevent checking with a half updated system
        //another solution would be double buffering
        for nname in firingnodes {
            let mut n = match nodes.get_mut(nname.as_str()) {
                Some(n) => n,
                _ => panic!("This should not happen"),
            };
            fireNode(&mut edges, &mut n);
            println!("{}\t :Fired node {}", i, n.name);
        }
        //build a json dictionary with all nodes and edges
        let mut package = format!("{{\"time\":{},\"edges\":[", i);
        for e in &edges {
            let j = serde_json::to_string(&e).unwrap();
            package = package + &j + ",";
        }
        package.pop(); //pop the last , because we are done with edges
        package = package + "],\"nodes\": [";
        for (_, n) in &nodes {
            let j = serde_json::to_string(&n).unwrap();
            package = package + &j + ",";
        }

        package.pop(); //pop the last , because we are done with nodes
        package = package + "]}";
        //publish the json dictionary on our zmq socket
        publisher.send(&package.as_bytes(), 0).unwrap();
        i += 1;
        thread::sleep(second);
    }
}
