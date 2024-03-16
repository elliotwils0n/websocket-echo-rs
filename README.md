# websocket-echo-rs

[The WebSocket Protocol](https://www.rfc-editor.org/rfc/rfc6455.html)

## Start server
```shell
cargo build
cargo run
```

## Connect to server (i.e. via console in browser's dev tools)

#### Create WebSocket
```javascript
let ws = new WebSocket("ws://localhost:8010");
ws.onopen = () => console.log("opened");
ws.onclose = () => console.log("closed");
ws.onmessage = (message) => console.log(message.data);
ws.onerror = (err) => console.log(err);
```

#### Send messages
```javascript
ws.send('żółć ęęęąąąćććóóóćśśććłłłóó');
```

#### Peacefully close connection
```javascript
ws.send('close');
```
