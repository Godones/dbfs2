all:
	@cargo run --release --example fuse -- --allow-other --auto-unmount --mount-point ./bench/dbfs
	@echo "run over"