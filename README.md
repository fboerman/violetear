# violetear
## Purpose
This repo contains the source for violetear, a dataflow graph simulator written in the programming language Rust. It is meant as a way for the author to learn writing Rust. It's released for education purpose.
## System Setup
The system is setup modular. There is the core dataflow simulator Violetear which reads from config files and executes the actual simulation. The results from this are published on a zmq socket. A webgui is provided with a python script who listens on the zmq socket, converts it to graphviz dot graph and pushes it on a websocket to a webgui. The webgui.html will simply render the given dotgraph.
### Config Format
There are two config files: ```nodes.yaml``` and ```layout.df```.

The first defines all the nodes in a yaml format with their properties. At the moment there is only one real property: the ammount of ticks firing takes for that node.

The second defines the layout of the dataflow graph and the initial tokens on each edge. There are two sections denoted with ```[layout]``` and ```[initial]``` to denote the edges and the initial tokens respectively.

Format for layout is: ```<nodename_from>:<intokens>-><outtokens>:<nodename_to>```

Format for the initial is:
```<nodename_from>-><nodename_to>:<initialtokens>```

### Name Origin
[https://en.wikipedia.org/wiki/Violetear](https://en.wikipedia.org/wiki/Violetear)


## Usage
### Requirements
* libzmq
* python3 with pyzmq installed
* Rust with cargo
* a webserver to host the webgui folder

### Running
First build the core binary with ```cargo build --release```. The binary will be located in ```target/release/```.

At the moment the websocket link is hardcoded to localhost, you can change it (and the port) on line 71 of webgui.py and line 20 of webgui.html. After this start the python script with ```python3 webgui.py```. After this start the rust binary and browse to where you host the webgui. You will now see the dataflow simulation in action.