# rusty-bank

[![license][license-badge]][license-url]

[license-badge]: https://img.shields.io/github/license/hseeberger/eventsourced
[license-url]: https://github.com/hseeberger/eventsourced/blob/main/LICENSE

Example application built with [eventsourced](https://github.com/hseeberger/eventsourced) using Event Sourcing and CQRS.

## Running

To run rusty-bank you eigher have to run [NATS](https://nats.io/) or [Postgres](https://www.postgresql.org/) or some other implementation of eventsoured's `EvtLog` and `SnapshotStore`.

### Using NATS

```
RUST_LOG=info,rusty_bank=debug,eventsourced=debug,eventsourced-nats=debug \
    cargo run
```

### Using Postgres

```
RUST_LOG=info,rusty_bank=debug,eventsourced=debug,eventsourced-postgres=debug \
    CONFIG_ENVIRONMENT=postgres \
    cargo run --no-default-features --features postgres
```

## License ##

This code is open source software licensed under the [Apache 2.0 License](http://www.apache.org/licenses/LICENSE-2.0.html).
