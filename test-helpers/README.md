# Test Helpers

This crate implements several utilities to help with network tests. 

Existent test helpers: 

- **Echo test helper**: It's an echo server, returns whatever it gets as input bytes. A good way to test it is telnet: `telnet localhost 1234`
- **Json test helper**: An http server that returns metadata about the request as a json

## Proxy server

This a simple nginx server that will add extra headers to the request. It's used for testing the json test helper.

You can run it with the following command: 

```
make run_proxy
```
