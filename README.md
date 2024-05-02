# Cron for Containers

[![Crates.io](https://img.shields.io/crates/v/cfc.svg)](https://crates.io/crates/cfc)
[![Coverage status](https://img.shields.io/codecov/c/github/Ayowel/cfc)](https://codecov.io/github/Ayowel/cfc/)
[![Apache License](https://img.shields.io/badge/license-APACHE%202.0-blue.svg)](https://www.apache.org/licenses/LICENSE-2.0.html)
[![Documentation](https://docs.rs/cfc/badge.svg)](https://docs.rs/cfc)

Cron for Containers is a lightweight cron daemon for containers
that aims to be an in-place replacement for ofelia wherever mail
capabilities are not required.

Currently, only docker and podman are supported.

## Installation

Install the executable with cargo:

```bash
cargo install cfc
```

## Usage

You may either provide a configuration file or extract configuration from container
labels.

The scheduling format is an "augmented" cron format inspired by go's implementation. E.g. `@every 10m`` or `0 10 * * * *`.

*Note:* The cron format does not have to contain the seconds specifier

You can configure four different kind of jobs:

`job-exec`: Executed in a running container.
`job-run`: Executed in a new container, using a specific image.
`job-local`: Executed on the host running ofelia.
`job-service-run`: Executed in a new "run-once" service, for running inside a swarm

### INI-style config

```ini
[job-exec "job-executed-on-running-container"]
schedule = @hourly
container = my-container
command = touch /tmp/cfc
```

### YAML-style config

```yaml
job-executed-on-running-container:
    kind: job-exec
    schedule: "@hourly"
    container: my-container
    command: touch /tmp/cfc
```

### Label-based config

```bash
docker run -it --rm \
  --label cfc.job-run.my-test-job.schedule="@every 5s" \
  --label cfc.job-run.my-test-job.command="echo Hello world" \
  alpine:latest sleep 9999
```

### Ofelia compatibility

Add `--ofelia` to the command-line when running cfc to run in compatibility mode.

## Note

Though both an executable and a library are made available, the library is only
intended for consumption by the executable and its API should not be considered stable.
