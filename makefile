release: hash btree sqlmem sql sur surmem

.PHONY: hash
hash:
	cargo build -r; cp target/release/db-faceoff hash

.PHONY: btree
btree:
	cargo build -r --features btree; cp target/release/db-faceoff btree

.PHONY: sqlmem
sqlmem:
	cargo build -r --features sqlite,mem; cp target/release/db-faceoff sqlmem

.PHONY: sql
sql:
	cargo build -r --features sqlite; cp target/release/db-faceoff sql

.PHONY: surmem
surmem:
	cargo build -r --features surreal,mem; cp target/release/db-faceoff surmem

.PHONY: sur
sur:
	cargo build -r --features surreal; cp target/release/db-faceoff sur

clean:
	-rm -f hash sqlmem sql surmem sur
	-rm -rf my.data*
	cargo clean
