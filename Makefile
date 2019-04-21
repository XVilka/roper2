
all: target/debug/roper


unicorn/libunicorn.so.1:
	cd unicorn
	./make.sh

target/debug/roper: unicorn/libunicorn.so.1
	cargo build

target/release/roper: unicorn/libunicorn.so.1
	cargo build --release

run:
	bash -c ". env.rc ; LD_LIBRARY_PATH=${LD_LIBRARY_PATH}:${PWD}/unicorn cargo run --release"

debug:
	bash -c ". env.rc ; LD_LIBRARY_PATH=${LD_LIBRARY_PATH}:${PWD}/unicorn cargo run"

