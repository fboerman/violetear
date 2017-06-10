import time
from time import sleep
import datetime
import struct
import socketserver
import hashlib
import base64

#helper functions for parsing and creating websocket frames
class GFunc():
    def __init__(self):
        pass

    #code: U unspeciefied, E error, N notice, S setting, D debug
    def log(self, txt, code="Unspecified"):
        ts = datetime.datetime.today().strftime("%Y-%m-%d %H:%M")
        print("["+str(ts)+"] ["+str(code[0])+"] "+str(txt))

    ###########Socket functions


    #new code inserted to handle data larger than 125
    # Stolen from http://www.cs.rpi.edu/~goldsd/docs/spring2012-csci4220/websocket-py.txt
    def create_frame(self, data, Opcode=1):
        # first byte, always send an entire message as one frame (fin)
        b1 = 0x80  # '0b10000000'

        # type of data
        b1 += Opcode  # if Opcode=1, b1 = '0b10000001'

        # make data
        if type(data) == str:
            payload = data.encode("UTF8")
        elif type(data) == bytes:
            payload = data
        else:
            return 'ERROR, data type not supported'

        # make first byte
        message = struct.pack(">B", b1)  # chr makes a ascii char from the byte. Sending this is equal to sending the bits

        # b2 starts with zero, because server>client is never masked
        # make mask and payload-length
        length = len(payload)
        if length < 126:
            b2 = length
            message += struct.pack(">B",b2)
        elif length < (2 ** 16) - 1:
            b2 = 126
            message += struct.pack(">B",b2)
            message += struct.pack(">H", length)
        elif length < (2 ** 64) - 1:
            b2 = 127
            message += struct.pack(">B",b2)
            message += struct.pack(">Q", length)
        else:
            return "ERROR, too long message"

        # add the message to the header
        message += payload

        # Send to the client
        return message


    #receive data from client
    def parse_frame(self,s,masked=True):
        # read the first two bytes, first contains fin bit and opcode, second contains payload length.
        frame_head = s.recv(2)

        # very first bit indicates if this is the final fragment
        # print("final fragment: ", self.is_bit_set(frame_head[0], 7))

        # bits 4-7 are the opcode (0x01 -> text)
        # print("opcode: ", frame_head[0] & 0x0f)

        # mask bit, from client will ALWAYS be 1, except python client
        if masked:
            assert self.is_bit_set(frame_head[1], 7)

        # length of payload
        # 7 bits, or 7 bits + 16 bits, or 7 bits + 64 bits
        payload_length = frame_head[1] & 0x7F
        if payload_length == 126:
            raw = s.recv(2)
            payload_length = self.bytes_to_int(raw)
        elif payload_length == 127:
            raw = s.recv(8)
            payload_length = self.bytes_to_int(raw)
        # print('Payload is {} bytes'.format(payload_length))

        #masking key
        #All frames sent from the client to the server are masked by a
        #32-bit nounce value that is contained within the frame
        if masked:
            masking_key = s.recv(4)
            # print("mask: ", masking_key, self.bytes_to_int(masking_key))

            #empty bytearray to fill with data
            data = bytearray(payload_length)

        # finally get the payload data:
        masked_data_in = s.recv(payload_length)

        if masked:
            # The ith byte is the XOR of byte i of the data with
            # masking_key[i % 4]
            for i, b in enumerate(masked_data_in):
                data[i] = b ^ masking_key[i%4]
        else:
            data = masked_data_in

        return data

    def is_bit_set(self, int_type, offset):
        mask = 1 << offset
        return not 0 == (int_type & mask)

    def set_bit(self, int_type, offset):
        return int_type | (1 << offset)

    def bytes_to_int(self, data):
        # note big-endian is the standard network byte order
        return int.from_bytes(data, byteorder='big')

    #def hash_password(password):
    #    # uuid is used to generate a random number
    #    salt = uuid.uuid4().hex
    #    return hashlib.sha256(salt.encode() + password.encode()).hexdigest() + ':' + salt

   # def check_password(hashed_password, user_password):
    #    password, salt = hashed_password.split(':')
     #   return password == hashlib.sha256(salt.encode() + user_password.encode()).hexdigest()


