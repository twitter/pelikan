build:
	cargo build

dict.cdb:
	scripts/mkcdb.sh

benchmark: dict.cdb
	cargo run --release --bin cdb_rs -- dict.cdb

clean:
	rm -f dict.cdb

.DEFAULT_GOAL := benchmark
.PHONY: build dict.cdb clean
