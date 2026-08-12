all:
	cargo build --release

ooechohelpd:
	cargo build --release --bin ooechohelpd

run_ooechohelpd:
	cargo run --bin ooechohelpd -- --port 1234

oojsonhelpd:
	cargo build --release --bin oojsonhelpd

run_oojsonhelpd:
	cargo run --bin oojsonhelpd -- --port 1234

# This is an nginx server that will add extra headers to the request.
# You can use it to test the json test helper
run_proxy:
	@command -v docker >/dev/null 2>&1 || (echo "docker not found"; exit 1)
	docker compose -f proxy/docker-compose.yml up
