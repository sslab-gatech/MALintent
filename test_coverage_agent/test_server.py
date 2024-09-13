import socketserver
import random


COVERAGE_MAP_SIZE = 64 * 1024
fake_map = bytearray([0] * COVERAGE_MAP_SIZE)


class MyTCPHandler(socketserver.BaseRequestHandler):
    def handle(self):
        # self.request is the TCP socket connected to the client
        while True:
            command = self.request.recv(1)
            if command == b'r':
                continue
            elif command == b'd':
                # send the coverage map.
                if random.choice([False, False, False, True]):
                    print("[!] Changing fake map")
                    fake_map[23] = fake_map[23] + 1
                self.request.sendall(fake_map)
            elif command == b'':
                print("[-] Client disconnected")
                break
            else:
                raise ValueError("Received incorrect command")

if __name__ == "__main__":
    HOST, PORT = "localhost", 6249

    with socketserver.TCPServer((HOST, PORT), MyTCPHandler) as server:
        server.serve_forever()
