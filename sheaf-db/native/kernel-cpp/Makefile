# Makefile - manual build of the Tier 1 kernel static library.
#
# The Rust build.rs normally drives this compilation automatically when it
# finds a toolchain. This Makefile is the manual path for inspection and for
# machines where the user prefers an explicit build:
#
#   make CXX=clang++          (LLVM)
#   make CXX=g++              (GCC 13+)
#   make CXX="zig c++"        (Zig bundled Clang)
#
# The library uses no C++ runtime features: no exceptions, no RTTI, no iostream.
# That keeps the static archive linkable into the MSVC or GNU Rust targets.

CXX ?= clang++
AR ?= ar
CXXFLAGS ?= -O3 -std=c++23 -fno-exceptions -fno-rtti -Wall -Wextra
# Runtime dispatch owns the ISA choice. Do not set -march=native here: the
# binary must stay runnable on CPUs without AVX-512 (cpu_features.cpp gates it).

SRC := $(wildcard src/*.cpp)
OBJ := $(patsubst src/%.cpp,build/%.o,$(SRC))
LIB := build/libsheaf_kernel.a

.PHONY: all clean
all: $(LIB)

build:
	mkdir -p build

build/%.o: src/%.cpp | build
	$(CXX) $(CXXFLAGS) -I../include -c $< -o $@

$(LIB): $(OBJ)
	$(AR) rcs $@ $(OBJ)

clean:
	rm -rf build