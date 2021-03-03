# Taper

Taper is a Rust library made for easy packet based network communication.

## Async

Taper supports async usage with the `async` feature.
When enabled, taper will use Tokio to execute concurrently all packet listener.
If disabled, OS Threads are used instead

While this may be overkill for Clients, this may be a great performance improvement for Servers.

## Documentation And Usage

[Documentation](https://docs.rs/taper)

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](https://choosealicense.com/licenses/mit/)
