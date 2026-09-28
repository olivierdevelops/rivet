#!/usr/bin/env python3
"""Local gRPC fixture for docs/demos/10-grpc (needs: pip install grpcio protobuf).

Usage: python fixtures/grpc_fixture.py DESCRIPTOR_SET PORT

Serves the example.Users service from schemas/users.proto over plaintext
HTTP/2 (h2c) on 127.0.0.1:PORT. Message classes are built at runtime from the
protoc FileDescriptorSet DESCRIPTOR_SET (the same users.pb Rivet loads), so no
generated *_pb2.py code is needed. Runs until killed.

Contracts (the README's fixture contracts):
  GetUser  id "42"            -> {id:"42", name:"Ada"}; other ids -> NOT_FOUND
  Watch    topic "changes"    -> "change 1", "change 2", then OK
           topic "fail_after_one" -> "change 1", then UNAVAILABLE
           other topics       -> no messages, then OK
  Upload   counts the client's messages -> {count:N}
  Chat     echoes each message as it arrives; finishes (OK) after the client
           half-closes
"""
import sys
from concurrent import futures

import grpc
from google.protobuf import descriptor_pb2, descriptor_pool, message_factory


def load(path):
    fds = descriptor_pb2.FileDescriptorSet()
    with open(path, "rb") as f:
        fds.ParseFromString(f.read())
    pool = descriptor_pool.DescriptorPool()
    for fd in fds.file:
        pool.Add(fd)
    return pool


def main():
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    pool = load(sys.argv[1])
    port = int(sys.argv[2])
    cls = lambda name: message_factory.GetMessageClass(pool.FindMessageTypeByName(name))
    GetUserRequest = cls("example.GetUserRequest")
    User = cls("example.User")
    WatchRequest = cls("example.WatchRequest")
    ChatMessage = cls("example.ChatMessage")
    UploadReply = cls("example.UploadReply")

    def get_user(req, ctx):
        if req.id == "42":
            return User(id="42", name="Ada")
        ctx.abort(grpc.StatusCode.NOT_FOUND, f"no user {req.id}")

    def watch(req, ctx):
        if req.topic == "changes":
            yield ChatMessage(text="change 1")
            yield ChatMessage(text="change 2")
        elif req.topic == "fail_after_one":
            yield ChatMessage(text="change 1")
            ctx.abort(grpc.StatusCode.UNAVAILABLE, "backend went away")

    def upload(it, ctx):
        return UploadReply(count=sum(1 for _ in it))

    def chat(it, ctx):
        for m in it:
            yield ChatMessage(text=m.text)

    ser = lambda m: m.SerializeToString()
    handlers = {
        "GetUser": grpc.unary_unary_rpc_method_handler(
            get_user, GetUserRequest.FromString, ser),
        "Watch": grpc.unary_stream_rpc_method_handler(
            watch, WatchRequest.FromString, ser),
        "Upload": grpc.stream_unary_rpc_method_handler(
            upload, ChatMessage.FromString, ser),
        "Chat": grpc.stream_stream_rpc_method_handler(
            chat, ChatMessage.FromString, ser),
    }
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=16))
    server.add_generic_rpc_handlers(
        (grpc.method_handlers_generic_handler("example.Users", handlers),))
    server.add_insecure_port(f"127.0.0.1:{port}")
    server.start()
    print(f"grpc fixture on http://127.0.0.1:{port} (example.Users)", flush=True)
    server.wait_for_termination()
    return 0


if __name__ == "__main__":
    sys.exit(main())
