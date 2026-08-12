all:
	cargo build --release

echoth:
	cargo build --release --bin echoth

run_echoth:
	cargo run --bin echoth -- --port 1234

jsonth:
	cargo build --release --bin jsonth

run_jsonth:
	cargo run --bin jsonth -- --port 1234

# This is an nginx server that will add extra headers to the request.
# You can use it to test the json test helper
run_proxy:
	@command -v docker >/dev/null 2>&1 || (echo "docker not found"; exit 1)
	docker compose -f proxy/docker-compose.yml up
