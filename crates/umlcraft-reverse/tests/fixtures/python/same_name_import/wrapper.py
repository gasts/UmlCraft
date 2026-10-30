import socket


class Wrapper:
    def __init__(self, sock: socket.socket):
        self._sock = sock
