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
// use std::String::StrSl
use yaml_rust::{YamlLoader, yaml};
use std::{thread, time};


#[derive(PartialEq)]
enum section {
    scanning,
    layout,
    initial,
}

#[derive(Default, Serialize, Deserialize)]
struct edge {
    from: String,
    to: String,
    tokensin: u32,
    tokensout: u32,
    currenholding: u32,
}
#[derive(Default, Serialize, Deserialize)]
struct node {
    name: String,
    time: u32,
}

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

fn findEdge<'a>(edges: &'a mut Vec<edge>, from: &String, to: &String) -> Option<&'a mut edge> {
    for e in edges {
        if e.from == *from && e.to == *to {
            return Some(e);
        }
    }

    None
}

fn checkNodeInputs(edges: &Vec<edge>, n: &node) -> bool {
    for e in edges {
        if e.to == n.name {
            //edge to our node so check if this edge has enough tokens to sustain firing node
            //if not than the node cannot fire
            if e.tokensout > e.currenholding {
                return false;
            }
        }
    }

    //all edges to this node have enough tokens so node can fire
    true
}

fn fireNode(edges: &mut Vec<edge>, n: &node) {
    for e in edges {
        if e.from == n.name {
            //found edge leaving the node, add tokens
            e.currenholding += e.tokensin;
        }
        if e.to == n.name {
            //foudn edge entering node, subtract tokens
            e.currenholding -= e.tokensout;
        }
    }
}

fn main() {

    //first load and parse the nodes from nodes.yaml config file
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
    let mut nodes = HashMap::new();

    match *yamlnodes {
        yaml::Yaml::Hash(ref h) => {
            for (nme, settings) in h {
                let n = node {
                    name: nme.as_str().unwrap().to_string().clone(),
                    time: settings["time"].as_i64().unwrap() as u32,
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

    let bufread = BufReader::new(file2);
    let mut status = section::scanning;
    for line in bufread.lines().filter_map(|result| result.ok()) {

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
            continue;
        }

        match status {
            section::layout => {
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
                E.currenholding = 0;
                edges.push(E);
            }
            section::initial => {
                let mut tuple = scanUntilDelimeter(&line, '-', 0);
                let from = tuple.1;
                tuple = scanUntilDelimeter(&line, ':', tuple.0 + 1);
                let to = tuple.1;
                let mut edg = match findEdge(&mut edges, &from, &to) {
                    Some(e) => e,
                    None => panic!("Parser error: non existing edge in  initial"),
                };
                tuple = scanUntilDelimeter(&line, ':', tuple.0);
                edg.currenholding = match tuple.1.parse() {
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
        // let update = format!("{}:time", i);
        // publisher.send(&update.as_bytes(), 0).unwrap();
        
        let mut firingnodes: Vec<&node> = Vec::new();

        //find all nodes that can currently fire
        for (_, n) in &nodes {
            if checkNodeInputs(&edges, &n){
                firingnodes.push(&n);
            }
        }
        //fire them
        for n in firingnodes {
            fireNode(&mut edges, n);
            println!("{}\t :Fired node {}",i, n.name);
            // let update = format!("{}:fired:{}",i, n.name);
        }
        
        let mut package = format!("{{\"time\":{},\"edges\":[", i);
        for e in &edges {
            let j = serde_json::to_string(&e).unwrap();
            package = package + &j + ",";
        }
        package.pop();
        package = package + "],\"nodes\": [";
        for (_, n) in &nodes {
            let j = serde_json::to_string(&n).unwrap();
            package = package + &j + ",";
        }

        package.pop();
        package = package + "]}";
        publisher.send(&package.as_bytes(), 0).unwrap();
        println!("{}", package);
        i += 1;
        thread::sleep(second);
    }
}
