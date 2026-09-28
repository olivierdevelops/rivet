# Build the librivet C examples, shared and static (PROP-2026-0002 T-11, T-19).
#
#   make -C examples/c                build demo, demo_static, modules, modules_static
#   make -C examples/c run            run the shared and static demos
#   make -C examples/c run-modules    run the shared and static module examples
#   make -C examples/c test           run everything
#   make -C examples/c TARGET=../../target/debug OUT=/tmp/x all   (another build, another output dir)
#
#   shared: cc demo.c -I ffi/include -L target/release -lrivet -Wl,-rpath,…
#   static: cc demo.c -I ffi/include target/release/librivet.a $(LIBS_PRIVATE)
#
# The library is built (cargo build --release -p rivet-ffi) only when it is missing.
# LIBS_PRIVATE is `cargo rustc -p rivet-ffi --release --crate-type staticlib -- --print
# native-static-libs` for this OS (also rivet.pc's Libs.private). On macOS, build the
# library and link the program with the same MACOSX_DEPLOYMENT_TARGET, or ld warns that
# objects were "built for newer macOS" (ring's assembly; harmless but noisy).

ROOT      := ../..
TARGET    ?= $(ROOT)/target/release
OUT       ?= .
INCLUDE   := $(ROOT)/ffi/include
CC        ?= cc
CFLAGS    ?= -Wall -Wextra -O2
UNAME     := $(shell uname -s)

ifeq ($(UNAME),Darwin)
  SHARED_LIB   := $(TARGET)/librivet.dylib
  LIBS_PRIVATE := -framework Security -framework CoreFoundation -liconv -lc -lm
else
  SHARED_LIB   := $(TARGET)/librivet.so
  LIBS_PRIVATE := -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
endif
RPATH      := -Wl,-rpath,$(abspath $(TARGET))
STATIC_LIB := $(TARGET)/librivet.a

.PHONY: all run run-modules test clean
all: $(OUT)/demo $(OUT)/demo_static $(OUT)/modules $(OUT)/modules_static

$(SHARED_LIB) $(STATIC_LIB):
	cargo build --release -p rivet-ffi --manifest-path $(ROOT)/Cargo.toml

$(OUT)/demo: demo.c $(SHARED_LIB)
	$(CC) $(CFLAGS) demo.c -I $(INCLUDE) -L $(TARGET) -lrivet $(RPATH) -o $@

$(OUT)/demo_static: demo.c $(STATIC_LIB)
	$(CC) $(CFLAGS) demo.c -I $(INCLUDE) $(STATIC_LIB) $(LIBS_PRIVATE) -o $@

$(OUT)/modules: modules.c $(SHARED_LIB)
	$(CC) $(CFLAGS) modules.c -I $(INCLUDE) -L $(TARGET) -lrivet $(RPATH) -o $@

$(OUT)/modules_static: modules.c $(STATIC_LIB)
	$(CC) $(CFLAGS) modules.c -I $(INCLUDE) $(STATIC_LIB) $(LIBS_PRIVATE) -o $@

run: $(OUT)/demo $(OUT)/demo_static
	$(OUT)/demo ../ffi/app.rivet
	$(OUT)/demo_static ../ffi/app.rivet

run-modules: $(OUT)/modules $(OUT)/modules_static
	$(OUT)/modules ../modules
	$(OUT)/modules_static ../modules

test: run run-modules

clean:
	rm -f $(OUT)/demo $(OUT)/demo_static $(OUT)/modules $(OUT)/modules_static
