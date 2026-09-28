# Capy Connection and Pipeline Runtime

## 1. Purpose

The runtime provides one declarative system for calling external capabilities and composing them into pipelines.

It should support:

- HTTP APIs
- HTTP streaming
- SSE
- WebSocket
- TCP sockets
- UDP sockets
- Unix domain sockets
- local CLI programs
- stdin/stdout protocols
- long-running subprocesses
- JSON-RPC
- raw binary protocols
- file I/O
- named pipes where supported
- polling APIs
- concurrent calls
- retries
- timeouts
- cancellation
- error handling
- output transformation
- pipelines composed from any combination of the above

The core principle is:

> The syntax should resemble the underlying protocol rather than hiding everything behind a generic `call()` abstraction.

A developer looking at:

```text
http post ...
```

should immediately know that an HTTP POST is happening.

Likewise:

```text
command ...
websocket ...
tcp ...
unix ...
udp ...
```

should expose the real mechanism being used.

---

# 2. Core model

Every executable operation follows the same conceptual lifecycle:

```text
configuration
    ↓
connect / start
    ↓
send input
    ↓
receive output
    ↓
transform
    ↓
validate
    ↓
return value
```

At runtime every operation resolves to:

```text
Operation<Input, Output, Error>
```

Examples:

```text
HTTP request
    → Response

CLI command
    → ProcessResult

WebSocket
    → Socket

TCP connection
    → Connection

Streaming HTTP
    → Iterable<Event>

Pipeline
    → arbitrary typed result
```

---

# 3. Values

The basic value types are:

```text
text
integer
number
bool
bytes
file
json
list
object
```

Special runtime values:

```text
response
socket
connection
process
stream
error
```

Objects use normal field access:

```text
response.user.name
response.items[0].url
event.delta.text
```

There is no JSONPath requirement for ordinary access.

---

# 4. Variables

Assignment:

```text
response = ...
```

Derived values:

```text
name = response.user.name
```

Mutation may be allowed where stateful behavior requires it:

```text
text = ""

for event in response
  text += event.delta
end
```

Pipelines may otherwise encourage single-assignment.

---

# 5. Parameters

```text
pipeline make_scene
  param prompt text required
  param width integer default 1280
  param height integer default 720
```

Additional constraints:

```text
param width integer min 64 max 4096
param voice text required
param tags list text
param options json
```

---

# 6. Secrets

Secrets should not be ordinary parameters.

```text
secret OPENAI_API_KEY from env
secret DB_PASSWORD from env
```

Usage:

```text
header "Authorization" "Bearer {{OPENAI_API_KEY}}"
```

Secrets must:

- never appear in logs
- never appear in generated manifests
- never appear in diagnostics
- never be persisted into outputs
- be redacted from errors

---

# 7. HTTP

## 7.1 GET

```text
response = http get "https://api.example.com/users/42"
  transform json
end

name = response.name
```

---

## 7.2 Query parameters

```text
response = http get "https://api.example.com/search"
  query q prompt
  query limit 20
  query page page

  transform json
end
```

Equivalent conceptual HTTP request:

```text
GET /search?q=...&limit=20&page=...
```

---

## 7.3 POST JSON

```text
response = http post "https://api.example.com/chat"
  header "Authorization" "Bearer {{API_KEY}}"

  body json """
  {
    "model": "model-name",
    "messages": [
      {
        "role": "user",
        "content": {{prompt}}
      }
    ]
  }
  """

  transform json
end

text = response.choices[0].message.content
```

---

# 8. HTTP body types

## JSON

```text
body json """
{
  "prompt": {{prompt}},
  "width": {{width}},
  "height": {{height}}
}
"""
```

---

## Text

```text
body text """
{{document}}
"""
```

---

## XML

```text
body xml """
<speak>
  <voice name="{{voice}}">
    {{text}}
  </voice>
</speak>
"""
```

---

## Form

```text
body form
  username username
  password password
end
```

---

## Multipart

```text
body multipart
  field model "whisper-1"
  field language language
  file audio audio "audio/wav"
end
```

---

## Raw bytes

```text
body bytes data
```

---

## Raw file

```text
body file archive
```

---

# 9. Response transformation

Raw HTTP responses contain:

```text
response.status
response.headers
response.body
```

A transform changes the body representation.

```text
response = http get url
  transform json
end
```

Then:

```text
response.user.name
response.items[0]
```

Supported transforms could include:

```text
transform json
transform text
transform bytes
transform xml
transform form
```

The HTTP metadata should remain available:

```text
response.status
response.headers
```

even after transformation.

---

# 10. HTTP error handling

```text
response = http post url
  body json """
  {
    "prompt": {{prompt}}
  }
  """

  if status == 401
    fail "unauthorized"
  end

  if status == 429
    retry 3 wait 2s
  end

  if status >= 500
    retry 2 wait 1s
  end

  if status >= 400
    fail "request_failed" body
  end

  transform json
end
```

`status` inside the HTTP block refers to the HTTP response status.

---

# 11. HTTP timeout

```text
response = http get url
  timeout 30s
  transform json
end
```

Timeout includes:

```text
DNS
connection
TLS
request transmission
response wait
body read
```

More advanced versions may distinguish:

```text
connect_timeout 5s
read_timeout 30s
timeout 60s
```

---

# 12. HTTP redirects

Default:

```text
redirect follow
redirect_limit 5
```

Alternative:

```text
redirect none
```

---

# 13. HTTP streaming

Streaming remains an HTTP request.

It is not a separate top-level transport.

```text
response = http post url
  body json """
  {
    "prompt": {{prompt}},
    "stream": true
  }
  """

  stream sse
  transform json
end
```

A streamed response becomes iterable:

```text
for event in response
  ...
end
```

---

# 14. SSE

```text
response = http post "https://api.example.com/chat"
  header "Authorization" "Bearer {{API_KEY}}"

  body json """
  {
    "prompt": {{prompt}},
    "stream": true
  }
  """

  stream sse
  transform json
  timeout 2m
end

text = ""

for event in response
  if event.type == "text.delta"
    text += event.delta
    emit event.delta
  end

  if event.type == "response.done"
    break
  end
end

return text
```

The SSE parser handles:

```text
event:
data:
id:
retry:
```

The transformed `data` becomes the event object.

---

# 15. JSONL / NDJSON streaming

Useful for local model servers.

```text
response = http post "http://127.0.0.1:11434/api/generate"
  body json """
  {
    "model": "llama",
    "prompt": {{prompt}},
    "stream": true
  }
  """

  stream jsonl
  transform json
end

text = ""

for chunk in response
  text += chunk.response

  if chunk.done
    break
  end
end
```

---

# 16. Raw byte streaming

Useful for audio, video, downloads and custom binary protocols.

```text
response = http get audio_url
  stream bytes
end

output = file temp "speech.mp3"

for chunk in response
  output.write chunk
end

return output
```

---

# 17. Line streaming

Useful for logs and some APIs.

```text
response = http get logs_url
  stream lines
end

for line in response
  emit line
end
```

---

# 18. WebSocket

WebSocket should be its own connection type because it is bidirectional and persistent.

```text
socket = websocket "wss://api.example.com/realtime"
  header "Authorization" "Bearer {{API_KEY}}"
  transform json
  timeout 10m
end
```

Sending:

```text
socket.send json """
{
  "type": "response.create",
  "prompt": {{prompt}}
}
"""
```

Receiving:

```text
event = socket.receive
```

---

# 19. WebSocket loop

```text
text = ""

while socket.open
  event = socket.receive timeout 30s

  if event.type == "response.text.delta"
    text += event.delta
    emit event.delta
  end

  if event.type == "response.done"
    break
  end

  if event.type == "error"
    fail "remote_error" event
  end
end

socket.close

return text
```

---

# 20. WebSocket binary data

```text
socket = websocket voice_url
end

socket.send bytes audio_chunk

message = socket.receive

if message.type == "binary"
  speaker.write message.data
end
```

---

# 21. WebSocket ping/pong

The runtime should manage protocol-level ping/pong automatically.

Optional application-level control:

```text
socket.ping
```

Connection status:

```text
socket.open
socket.closed
socket.error
```

---

# 22. Reconnection

```text
socket = websocket url
  reconnect 5
  backoff exponential
  max_delay 30s
end
```

The runtime should distinguish between:

```text
connection lost
authentication failed
remote close
user cancellation
timeout
```

---

# 23. TCP sockets

TCP is useful for:

- custom servers
- database-like services
- embedded systems
- game engines
- local daemons
- legacy protocols

Opening:

```text
conn = tcp "127.0.0.1:9000"
  timeout 10s
end
```

Writing:

```text
conn.write text "HELLO\n"
```

Reading:

```text
response = conn.read line
```

Closing:

```text
conn.close
```

---

# 24. TCP request/response

```text
conn = tcp "127.0.0.1:9000"
  timeout 10s
end

conn.write text """
RENDER scene01
"""

result = conn.read line

conn.close

return result
```

---

# 25. TCP JSON protocol

```text
conn = tcp "127.0.0.1:9000"
  framing newline
  transform json
end

conn.send {
  "action": "render",
  "scene": scene,
}

response = conn.receive

if response.error
  fail "render_failed" response.error
end

return response.output
```

---

# 26. TCP framing

Raw TCP has no message boundaries, so framing must be explicit.

Supported framing:

```text
framing newline
framing length32
framing delimiter "\0"
framing raw
```

Example:

```text
conn = tcp host
  framing length32
  transform json
end
```

The runtime handles:

```text
[length: uint32][payload]
```

---

# 27. TLS over TCP

```text
conn = tcp "example.com:443"
  tls true
end
```

Advanced:

```text
tls {
  server_name "example.com"
  ca_file ca
}
```

Mutual TLS:

```text
tls {
  cert client_cert
  key client_key
  ca_file ca
}
```

---

# 28. UDP

UDP needs separate semantics because there is no persistent stream.

```text
socket = udp "127.0.0.1:7000"
end

socket.send bytes packet

response = socket.receive timeout 2s
```

Useful for:

- discovery
- telemetry
- OSC
- low-latency control
- embedded devices
- game/network protocols

---

# 29. UDP JSON

```text
socket = udp "192.168.1.50:8000"
  transform json
end

socket.send {
  "command": "status"
}

response = socket.receive timeout 1s

return response
```

---

# 30. UDP multicast

```text
socket = udp multicast "239.0.0.1:5000"
  interface "eth0"
end

for packet in socket
  emit packet
end
```

---

# 31. Unix domain sockets

Unix sockets should behave similarly to TCP but take a filesystem path.

```text
conn = unix "/var/run/myservice.sock"
end
```

Then:

```text
conn.write text "STATUS\n"
response = conn.read line
conn.close
```

---

# 32. Unix socket with JSON

```text
conn = unix "/tmp/renderd.sock"
  framing newline
  transform json
end

conn.send {
  "action": "render",
  "scene": scene,
}

response = conn.receive

return response.file
```

This is particularly useful for local services because it avoids TCP ports entirely.

---

# 33. HTTP over Unix socket

Some local services expose HTTP over a Unix domain socket.

Example conceptual syntax:

```text
response = http get "http://localhost/info"
  unix "/var/run/service.sock"
  transform json
end
```

This means:

```text
HTTP semantics
+
Unix socket transport
```

rather than TCP.

Docker-style APIs are a typical example of this architecture.

---

# 34. Named pipes

On platforms that support them:

```text
pipe = pipe "\\\\.\\pipe\\render-service"
end
```

or Unix FIFO:

```text
pipe = pipe "/tmp/render.pipe"
end
```

Usage:

```text
pipe.write request
response = pipe.read
```

This can share the same stream abstraction as sockets.

---

# 35. CLI / subprocess execution

CLI invocation should use explicit argv, not shell strings.

```text
result = command
  binary "/usr/bin/ffmpeg"

  args [
    "-i", input,
    "-vf", "scale=1280:720",
    "-c:v", "libx264",
    output
  ]

  timeout 1m
end
```

---

# 36. CLI result

A command produces:

```text
result.stdout
result.stderr
result.exit
result.duration
```

Example:

```text
if result.exit != 0
  fail "ffmpeg_failed" result.stderr
end
```

---

# 37. CLI result transformation

```text
result = command
  binary "/usr/bin/ffprobe"

  args [
    "-v", "quiet",
    "-print_format", "json",
    "-show_streams",
    video
  ]

  transform stdout json
end

width = result.stdout.streams[0].width
```

---

# 38. Environment variables

```text
result = command
  binary "/usr/local/bin/model-runner"

  args [
    "--model", model,
    "--input", input
  ]

  env {
    "CUDA_VISIBLE_DEVICES": "0"
    "MODEL_CACHE": cache_dir
    "API_KEY": secret_key
  }
end
```

---

# 39. stdin

```text
result = command
  binary "/usr/local/bin/piper"

  args [
    "--model", model,
    "--output_file", output
  ]

  stdin text
end
```

---

# 40. Streaming stdout

Some CLI programs produce continuous output.

```text
process = command
  binary "/usr/bin/tail"
  args ["-f", log_file]
  stream stdout lines
end

for line in process.stdout
  emit line
end
```

---

# 41. Streaming JSON from a CLI

```text
process = command
  binary "./worker"
  args ["--stream"]
  stream stdout jsonl
  transform stdout json
end

for event in process.stdout
  if event.type == "progress"
    emit event.progress
  end
end
```

---

# 42. Interactive subprocesses

Some tools need stdin and stdout simultaneously.

```text
process = command
  binary "./interactive-agent"
  interactive true
  transform stdout jsonl
end
```

Send:

```text
process.stdin.send {
  "prompt": prompt
}
```

Receive:

```text
response = process.stdout.receive
```

Shutdown:

```text
process.close
```

---

# 43. Shell execution

Shell should not be the default.

Potential explicit escape hatch:

```text
result = shell """
ffmpeg -i "$INPUT" "$OUTPUT"
"""
```

This should require explicit trust because shell parsing enables:

- expansion
- pipes
- redirection
- globbing
- command substitution

Normal `command` should remain preferred.

---

# 44. JSON-RPC over HTTP

```text
response = http post rpc_url
  body json """
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "render",
    "params": {
      "scene": {{scene}}
    }
  }
  """

  transform json
end

if response.error
  fail "rpc_failed" response.error
end

return response.result
```

---

# 45. JSON-RPC over WebSocket

```text
socket = websocket rpc_url
  transform json
end

socket.send {
  "jsonrpc": "2.0",
  "id": 10,
  "method": "generate",
  "params": {
    "prompt": prompt
  }
}

response = socket.receive

if response.error
  fail "rpc_failed" response.error
end

return response.result
```

---

# 46. JSON-RPC over Unix socket

```text
conn = unix "/tmp/engine.sock"
  framing newline
  transform json
end

conn.send {
  "jsonrpc": "2.0",
  "id": 1,
  "method": "render",
  "params": {
    "scene": scene
  }
}

response = conn.receive

return response.result
```

This demonstrates why protocol and transport should remain separate concepts.

---

# 47. Protocol layering

Conceptually:

```text
transport
    tcp
    unix
    websocket
    stdin/stdout
    http

framing
    raw
    lines
    jsonl
    length-prefixed
    websocket frames

encoding
    bytes
    text
    json
    xml
    protobuf

application protocol
    HTTP
    JSON-RPC
    custom
    LLM events
```

The user-facing language does not need to expose all layers constantly.

Defaults should make common cases short.

---

# 48. Polling APIs

Polling represents repeated requests toward a terminal condition.

```text
job = http post job_url
  body json """
  {
    "prompt": {{prompt}}
  }
  """

  transform json
end

result = poll every 5s timeout 10m
  response = http get "https://api.example.com/jobs/{{job.id}}"
    transform json
  end

  if response.status == "failed"
    fail "generation_failed" response.error
  end

  until response.status == "done"

  return response
end

return result.output
```

---

# 49. Poll backoff

```text
result = poll
  every 2s
  backoff exponential
  max_interval 30s
  jitter true
  timeout 10m

  response = http get status_url
    transform json
  end

  until response.done
  return response
end
```

---

# 50. Iterative LLM processing

Iteration is application-level logic, unlike transport streaming.

```text
pipeline improve
  param prompt text required

  answer = prompt

  iterate max 5
    response = http post llm_url
      body json """
      {
        "input": {{answer}}
      }
      """

      transform json
    end

    improved = response.output

    if improved == answer
      break
    end

    answer = improved
  end

  return answer
end
```

---

# 51. LLM tool loop

```text
pipeline agent
  param prompt text required

  messages = [
    {
      "role": "user",
      "content": prompt
    }
  ]

  iterate max 10
    response = http post llm_url
      body json """
      {
        "messages": {{messages}},
        "tools": {{tools}}
      }
      """

      transform json
    end

    if response.type == "final"
      return response.text
    end

    if response.type == "tool_call"
      result = tool response.tool.name
        args response.tool.arguments
      end

      messages += {
        "role": "tool",
        "id": response.tool.id,
        "content": result
      }
    end
  end

  fail "iteration_limit"
end
```

---

# 52. Concurrent requests

Independent operations should be able to run together.

```text
concurrent timeout 2m
  image = http post image_url
    body json """
    {
      "prompt": {{prompt}}
    }
    """

    transform json
  end

  audio = http post speech_url
    body json """
    {
      "text": {{narration}}
    }
    """

    returns audio mp3 from body
  end
end
```

Both operations begin simultaneously.

---

# 53. Shared timeout semantics

For:

```text
concurrent timeout 30s
  a = ...
  b = ...
  c = ...
end
```

the 30 seconds is one shared deadline.

Not:

```text
30 seconds per child
```

but:

```text
group start         t=0
a complete           t=4
b complete          t=15
c still running     t=30
                   ↓
cancel c
group timeout
```

---

# 54. Failure policies

Fail immediately:

```text
concurrent fail fast timeout 30s
  ...
end
```

Continue independent operations:

```text
concurrent fail independent timeout 30s
  ...
end
```

---

# 55. Concurrency limits

```text
concurrent limit 4 timeout 10m
  for scene in scenes
    renders[scene] = render scene
  end
end
```

No more than four operations run simultaneously.

---

# 56. Cancellation

Long-running resources should share a common cancellation model.

```text
request.cancel
socket.close
process.cancel
poll.cancel
group.cancel
```

A pipeline cancellation propagates to child operations by default.

---

# 57. Error model

All runtime failures resolve into a common structured error:

```text
error.kind
error.message
error.source
error.code
error.details
```

Potential kinds:

```text
timeout
connection
dns
tls
http
protocol
process
exit
parse
validation
cancelled
permission
not_found
unsupported
```

---

# 58. HTTP errors

```text
if status == 404
  fail "not_found"
end

if status == 429
  retry 3 wait 2s
end

if status >= 400
  fail "api_failed" body
end
```

---

# 59. Process errors

```text
if exit == 127
  fail "command_not_found"
end

if exit != 0
  fail "command_failed" stderr
end
```

---

# 60. Socket errors

```text
event, err = socket.receive

if err
  if err.kind == "connection"
    reconnect
  else
    return err
  end
end
```

---

# 61. try / catch

For pipeline-level recovery:

```text
try
  image = generate_image prompt
catch "generation_failed"
  image = fallback_image
end
```

Generic:

```text
try
  result = risky_operation
catch err
  log err
  return fallback
end
```

---

# 62. Finally / cleanup

```text
socket = websocket url

try
  ...
finally
  socket.close
end
```

Useful for:

- sockets
- temporary files
- processes
- transactions
- resources

---

# 63. File operations

Basic file primitives are useful because many external tools communicate through files.

```text
data = file.read source
file.write output data
```

Streaming:

```text
reader = file.open source

for chunk in reader
  ...
end
```

---

# 64. File watching

Potential future primitive:

```text
watcher = watch directory

for event in watcher
  if event.type == "created"
    process event.path
  end
end
```

This fits the same iterable resource model.

---

# 65. Full multimodal pipeline

```text
pipeline make_scene
  param prompt text required
  param narration text required

  concurrent timeout 2m

    image_response = http post image_api
      header "Authorization" "Bearer {{IMAGE_KEY}}"

      body json """
      {
        "prompt": {{prompt}},
        "width": 1280,
        "height": 720
      }
      """

      if status >= 400
        fail "image_failed" body
      end

      transform json
    end


    audio = http post speech_api
      header "Authorization" "Bearer {{SPEECH_KEY}}"

      body json """
      {
        "text": {{narration}}
      }
      """

      if status >= 400
        fail "speech_failed" body
      end

      returns audio mp3 from body
    end

  end

  image = base64 decode image_response.data[0].image

  if os == "macos"
    ffmpeg = "/opt/homebrew/bin/ffmpeg"
  else if os == "linux"
    ffmpeg = "/usr/bin/ffmpeg"
  else if os == "windows"
    ffmpeg = "C:\\ffmpeg\\bin\\ffmpeg.exe"
  else
    fail "unsupported_os"
  end

  video = command
    binary ffmpeg

    args [
      "-loop", "1",
      "-i", image,
      "-i", audio,
      "-shortest",
      output
    ]

    timeout 1m

    if exit != 0
      fail "video_encode_failed" stderr
    end

    returns video mp4 from output
  end

  return video
end
```

---

# 66. Full async video-generation pipeline

```text
pipeline generate_video
  param prompt text required

  job = http post video_api
    header "Authorization" "Bearer {{VIDEO_KEY}}"

    body json """
    {
      "prompt": {{prompt}}
    }
    """

    if status == 429
      retry 3 wait 5s
    end

    if status >= 400
      fail "submit_failed" body
    end

    transform json
  end

  task = poll
    every 5s
    timeout 10m
    backoff exponential
    max_interval 20s

    response = http get "{{video_api}}/{{job.id}}"
      header "Authorization" "Bearer {{VIDEO_KEY}}"
      transform json
    end

    if response.status == "failed"
      fail "generation_failed" response.error
    end

    until response.status == "completed"

    return response
  end

  video = http get task.output.url
    stream bytes
  end

  destination = file temp "video.mp4"

  for chunk in video
    destination.write chunk
  end

  return destination
end
```

---

# 67. Realtime voice pipeline

```text
pipeline voice_chat
  param mic stream required

  socket = websocket realtime_url
    header "Authorization" "Bearer {{API_KEY}}"
    transform json
  end

  concurrent timeout 30m

    sender = pipeline
      for audio in mic
        socket.send {
          "type": "audio.append",
          "audio": base64 encode audio
        }
      end
    end

    receiver = pipeline
      while socket.open
        event = socket.receive

        if event.type == "audio.delta"
          speaker.write base64 decode event.audio
        end

        if event.type == "error"
          fail "voice_session_failed" event
        end
      end
    end

  end

  socket.close
end
```

---

# 68. Local engine over Unix socket

```text
pipeline local_render
  param scene json required

  conn = unix "/tmp/render-engine.sock"
    framing newline
    transform json
  end

  conn.send {
    "type": "render",
    "scene": scene
  }

  response = conn.receive timeout 5m

  if response.status == "error"
    fail "render_failed" response.error
  end

  conn.close

  return response.output
end
```

---

# 69. Local engine over TCP

The same application protocol can use TCP instead:

```text
conn = tcp "127.0.0.1:7000"
  framing newline
  transform json
end
```

Everything after connection creation can remain identical.

This suggests the internal architecture should separate:

```text
transport
protocol
serialization
```

---

# 70. CLI connector protocol

A generic executable can expose a structured protocol through stdin/stdout.

```text
process = command
  binary "./my-connector"
  interactive true
  stream stdout jsonl
  transform stdout json
end

process.stdin.send {
  "action": "generate",
  "prompt": prompt
}

for event in process.stdout
  if event.type == "progress"
    emit event.progress
  end

  if event.type == "result"
    result = event.result
    break
  end
end

process.close

return result
```

This becomes the escape hatch for integrations too complex for declarative primitives.

---

# 71. Architecture

Internally the runtime should not implement every operation independently.

Use a common resource hierarchy:

```text
Runtime
│
├── HTTP Engine
│   ├── request
│   ├── streaming response
│   ├── SSE decoder
│   └── HTTP-over-Unix
│
├── Socket Engine
│   ├── WebSocket
│   ├── TCP
│   ├── UDP
│   ├── Unix socket
│   └── named pipe
│
├── Process Engine
│   ├── command
│   ├── stdin
│   ├── stdout/stderr
│   └── streaming process
│
├── Codec Layer
│   ├── JSON
│   ├── JSONL
│   ├── text
│   ├── bytes
│   ├── XML
│   ├── base64
│   └── framing
│
├── Execution Engine
│   ├── pipelines
│   ├── concurrent groups
│   ├── polling
│   ├── iteration
│   ├── retries
│   ├── timeout
│   └── cancellation
│
└── Type/Validation Engine
    ├── params
    ├── outputs
    ├── transforms
    └── protocol compatibility
```

---

# 72. Common runtime interfaces

Internally, transports can implement a few core traits.

Conceptually:

```text
Request
  execute() → Value

Stream
  next() → Value

Duplex
  send(Value)
  receive() → Value

Closable
  close()

Cancelable
  cancel()
```

Mapping:

```text
HTTP normal        Request
HTTP streaming     Stream
WebSocket          Duplex + Stream + Closable
TCP                Duplex + Closable
Unix socket        Duplex + Closable
UDP                Datagram Duplex
CLI                Request
interactive CLI    Duplex + Stream + Closable
poll               Request loop
pipeline           Request
```

This gives the runtime consistency without forcing the source syntax to look generic.

---

# 73. Resource ownership

Every resource belongs to a scope.

Example:

```text
pipeline foo
  socket = websocket url

  ...
end
```

If the pipeline exits without explicitly closing the socket, the runtime closes it automatically.

The same applies to:

```text
processes
TCP sockets
Unix sockets
file handles
temporary files
streams
```

Explicit close remains preferable when lifecycle matters.

---

# 74. Timeout hierarchy

Timeouts should propagate downward.

```text
pipeline timeout 10m
```

contains:

```text
poll timeout 5m
```

contains:

```text
http timeout 30s
```

Actual deadline is:

```text
min(
  pipeline remaining time,
  poll remaining time,
  HTTP timeout
)
```

This gives the runtime a shared deadline model.

---

# 75. Cancellation hierarchy

If the outer pipeline is cancelled:

```text
pipeline
   ↓ cancel
concurrent
   ↓
HTTP requests      cancelled
CLI processes      terminated
WebSockets         closed
TCP sockets        closed
poll loops         stopped
child pipelines    cancelled
```

No orphaned execution should remain by default.

---

# 76. Retries

Retries belong to operations where repeating is meaningful.

HTTP:

```text
retry 3 on status [429, 500, 502, 503]
```

Connection:

```text
reconnect 5
```

Poll:

```text
backoff exponential
```

A command should not automatically retry unless explicitly requested:

```text
retry 2 on exit [75]
```

because commands may have side effects.

---

# 77. Idempotency

The runtime should not assume that POST or commands are safe to retry.

Potential syntax:

```text
retry 3
  when status == 429
  idempotent true
end
```

Or simply require explicit retry declarations.

---

# 78. Validation

Validation occurs at three levels.

## Load

Check:

```text
syntax
known keywords
type declarations
valid transforms
valid connection options
```

## Compile

Check:

```text
variable references
branch consistency
returned types
pipeline dependencies
transport/operation compatibility
```

## Runtime

Check:

```text
connection availability
secrets
actual response
status code
exit code
timeouts
output type
```

---

# 79. Security

The runtime executes external processes and network calls, so tools/pipelines need a trust model.

Capabilities should include:

```text
network
process
filesystem
unix_socket
tcp
udp
websocket
environment
```

A project could request:

```text
permissions
  network ["api.openai.com", "api.example.com"]
  command ["/usr/bin/ffmpeg"]
  unix ["/tmp/render.sock"]
end
```

Anything outside those capabilities is rejected.

---

# 80. No implicit shell

This:

```text
args [
  "-i", input,
  "-vf", filter,
  output
]
```

must always become direct argv.

User-controlled values cannot turn into:

```text
new flags
pipes
redirection
subcommands
shell expansions
```

unless explicit `shell` mode is used.

---

# 81. Connection taxonomy

The language ultimately has a small number of recognizable primitives:

```text
http
websocket
tcp
udp
unix
pipe
command
file
```

And orchestration primitives:

```text
pipeline
concurrent
poll
iterate
for
while
if
try
```

And common behavior:

```text
timeout
retry
transform
fail
return
emit
```

That is enough to cover a very large range of external integrations without introducing a generic opaque connector abstraction.

---

# 82. Design principle

The language should prefer:

```text
response = http post ...
```

over:

```text
response = call("http", ...)
```

Prefer:

```text
socket = websocket ...
```

over:

```text
connection = connect({
  protocol: "websocket"
})
```

Prefer:

```text
result = command
  binary ...
  args [...]
end
```

over:

```text
result = execute({
  transport: "process"
})
```

The internal runtime may unify all these operations.

The surface language should not.

The user should always be able to recognize **what is actually happening**.

---

# 83. Recommended first implementation scope

The first version should implement:

```text
pipeline
param
if
for

http
  get/post/put/patch/delete
  query
  headers
  JSON/text/form/multipart/file bodies
  transform json/text/bytes
  stream sse/jsonl/bytes
  timeout
  retries

command
  binary
  args
  env
  stdin
  stdout/stderr
  timeout
  exit codes

websocket
  send
  receive
  JSON/text/binary
  timeout

tcp
unix
  read/write
  framing
  transform

concurrent
poll

try/fail
return
```

Then add:

```text
UDP
named pipes
interactive subprocesses
TLS customization
mTLS
binary framing
iterate
custom protocols
shell escape hatch
```

The important part is that the execution model is designed for those from the beginning, even if they are implemented later.

---

# 84. Final conceptual model

```text
                           PIPELINE
                              │
         ┌────────────────────┼────────────────────┐
         │                    │                    │
      Network              Process              Local
         │                    │                    │
   ┌─────┼───────┐         command              file
   │     │       │
 HTTP   WS    sockets
   │            │
   │       ┌────┼────┐
   │       TCP  UDP  Unix
   │
   ├── normal
   ├── SSE
   ├── JSONL
   └── bytes

                              │
                              ▼

                       Common execution
                    ┌───────────────────┐
                    │ timeout           │
                    │ cancellation      │
                    │ retry             │
                    │ errors            │
                    │ transformation    │
                    │ concurrency       │
                    │ validation        │
                    └───────────────────┘
                              │
                              ▼
                           RESULT
```

The strongest architectural choice is to make **protocols recognizable in the source language while unifying lifecycle behavior underneath**.

That gives you both readability and a clean runtime implementation.