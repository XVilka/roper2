
unicorn/libunicorn.so.1:
	cd unicorn
	./make.sh

target/debug/roper:
	cargo build

target/release/roper:
	cargo build --release

run:
	sh -c "LD_LIBRARY_PATH=${LD_LIBRARY_PATH}:${PWD}/unicorn cargo run --release"

debug:
	sh -c "LD_LIBRARY_PATH=${LD_LIBRARY_PATH}:${PWD}/unicorn cargo run"

