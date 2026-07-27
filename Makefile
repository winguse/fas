.PHONY: run test check fmt clippy clean

run:
	@echo "Starting FAS server on port 8080..."
	@echo "Access the admin page at http://localhost:8080/"
	FAS_ACL_FILE=acl.yaml FAS_DATA_FILE=fas.jsonl FAS_PORT=8080 cargo run

test:
	cargo test

check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings

clean:
	cargo clean
	rm -f fas.jsonl
