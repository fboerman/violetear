import zmq
import socketserver
from GFunc import *
import threading
import hashlib
import base64
import json

connections = []

class ThreadedTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
    pass
#simple implemenation of websocket server
class ThreadedServerHandler(socketserver.BaseRequestHandler):
    def handle(self):
        global connections

        self.GF = GFunc()
        data = self.request.recv(1024)
        addr = self.request.getpeername()[0]
        if "upgrade: websocket" in str(data, "utf-8").lower():
            self.HandShake(str(data, "utf-8"))
        else:
            print("no websocket request")
            return
        
        connections.append(self)

        while True:
            try:
                data = self.GF.parse_frame(self.request).decode()
            except:
                print("connection lost")
                connections.remove(self)
                return

            print(data)
            self.request.sendall(self.GF.create_frame(data))

    def SendClient(self, msg):
        self.request.sendall(self.GF.create_frame(msg.encode()))

    def HandShake(self, request):
        specificationGUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
        websocketkey = ""
        protocol = ""
        for line in request.split("\r\n"):
            if "Sec-WebSocket-Key:" in line:
                websocketkey = line.split(" ")[1]
            elif "Sec-WebSocket-Protocol" in line:
                protocol = line.split(":")[1].strip().split(",")[0].strip()
            elif "Origin" in line:
                self.origin = line.split(":")[0]
        fullKey = hashlib.sha1(websocketkey.encode("utf-8") + specificationGUID.encode("utf-8")).digest()
        acceptKey = base64.b64encode(fullKey)
        if protocol != "":
            handshake = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Protocol: " + protocol + "\r\nSec-WebSocket-Accept: " + str(acceptKey, "utf-8") + "\r\n\r\n"
        else:
            handshake = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: " + str(
                acceptKey, "utf-8") + "\r\n\r\n"
        self.request.send(bytes(handshake, "utf-8"))

if __name__ == "__main__":
    #connect to the zmq socket and start a websocket server
    context = zmq.Context()
    socket = context.socket(zmq.SUB)

    socket.connect("tcp://127.0.0.1:5556")
    socket.setsockopt_string(zmq.SUBSCRIBE, "")

    server = ThreadedTCPServer(("127.0.0.1", 8080), ThreadedServerHandler)
    server_thread = threading.Thread(target=server.serve_forever)
    server_thread.daemon = True
    server_thread.start()
   
    while True:
        #receive the json package
        broadcast = socket.recv_json()
        #convert the dictionary to a graphviz dot format
        graph = "digraph G{"
        for edge in broadcast["edges"]:
            label = '    i:{}, o:{}, c:{}'.format(edge["tokensin"], edge["tokensout"], edge["currentholding"])
            graph += '"{}"->"{}"[label="{}"];'.format(edge["from"], edge["to"], label)
        for node in broadcast["nodes"]:
            graph = graph.replace('"{}"'.format(node["name"]), '"{}:{}"'.format(node["name"], node["firing"]))
        graph += "}"
        #and send it to all connected clients
        for c in connections:
            c.SendClient(graph)
