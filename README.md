# Robot Admin

A gRPC-based game server administration tool with a web interface, built in Rust.

## Features

- gRPC server for game control and monitoring
- Web interface for easy administration
- Cross-platform support (Windows, Linux, macOS)
- Real-time game state monitoring
- Secure communication using gRPC

## Prerequisites

- Rust 1.70 or higher
- Protocol Buffers compiler (protoc)
  - Windows: Download from [protobuf releases](https://github.com/protocolbuffers/protobuf/releases)
  - macOS: `brew install protobuf`
  - Linux: `apt-get install protobuf-compiler`

## Installation

1. Clone the repository:
```bash
git clone https://github.com/duwei/robot_admin.git
cd robot_admin
```

2. Build the project:
```bash
cargo build --release
```

The compiled binary will be available in `target/release/`.

## Usage

1. Start the server:
```bash
./target/release/robot_admin
```

2. Open your web browser and navigate to:
```
http://localhost:3000
```

3. Testing with gRPC client:

You can test the gRPC service using our built-in test client, grpcurl, or BloomRPC.

Using test_client:
```bash
# Build and run the test client
cargo run --bin test_client

# Available commands in test client:
- get_status : Get the status of all games
- start_game <game_id> : Start a new game with specified ID
- stop_game <game_id> : Stop a running game
- list_games : List all running games
- help : Show this help message
- quit : Exit the client
```

Using grpcurl:
```bash
# List available services
grpcurl -plaintext localhost:50051 list

# List methods in the GameControl service
grpcurl -plaintext localhost:50051 list game_control.GameControl

# Get game status
grpcurl -plaintext localhost:50051 game_control.GameControl/GetGameStatus '{}'

# Start game
grpcurl -plaintext localhost:50051 game_control.GameControl/StartGame '{
  "game_id": "game1",
  "config": {
    "max_players": 10,
    "map_name": "default"
  }
}'

# Stop game
grpcurl -plaintext localhost:50051 game_control.GameControl/StopGame '{
  "game_id": "game1"
}'
```

Using the Web Interface:
1. Navigate to `http://localhost:3000`
2. Use the dashboard to:
   - View active games
   - Start new game instances
   - Monitor game status
   - Stop running games
   - View server statistics

## Project Structure

- `src/`: Source code directory
  - `grpc.rs`: gRPC server implementation
  - `main.rs`: Application entry point
- `proto/`: Protocol Buffers definitions
  - `game_control.proto`: Game control service definitions
- `web/`: Web interface files
- `.github/workflows/`: GitHub Actions CI/CD configuration

## Building from Source

The project uses GitHub Actions for automated builds. Each release includes pre-built binaries for:
- Windows (64-bit)
- Linux (64-bit)
- macOS (64-bit)

To build manually:

```bash
# Install protobuf compiler (if not already installed)
# Then build the project
cargo build --release
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contact

Project Link: [https://github.com/duwei/robot_admin](https://github.com/duwei/robot_admin)
