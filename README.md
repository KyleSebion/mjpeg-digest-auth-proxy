mjpeg-digest-auth-proxy 1.0.9
```
Usage: mjpeg-digest-auth-proxy [OPTIONS] <URL>

Arguments:
  <URL>  upstream mjpeg url

Options:
  -b, --binding <BINDING>    [default: 127.0.0.1:11111]
  -u, --username <USERNAME>  upstream mjpeg server username [env: MDAP_USERNAME=]
                             [default: username]
  -p, --password <PASSWORD>  upstream mjpeg server password [env: MDAP_PASSWORD=]
                             [default: password]
  -i, --insecure             allow insecure upstream server connections
  -l, --log-dir[=<LOG_DIR>]  enable logging to daily file. supply a value to override the
                             default log directory [default: logs]
  -h, --help                 Print help
  -V, --version              Print version
```
