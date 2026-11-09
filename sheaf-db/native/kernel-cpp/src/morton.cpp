// morton.cpp - Z-order coordinate keys and 64x64 cache-blocked matvec.
//
// Morton keys interleave the bits of two coordinates. Sorting tile indices
// by their Morton key makes consecutive tiles share cache lines, which is
// the space-filling-curve locality argument from spec Pillar 1. A 64 x 64
// f32 tile occupies 16 KB, so one tile plus its input slice fits in a 32 KB
// L1 data cache with room for the accumulator row.

#include "internal.hpp"

#include <algorithm>
#include <vector>

namespace sheaf::kernel {
namespace {

// Spread the 32 bits of x into the even bit positions of a 64-bit word.
// Each step doubles the gap between bits with a mask-and-shift network.
inline uint64_t spread_bits(uint32_t x) {
    uint64_t v = x;
    v = (v | (v << 16)) & 0x0000FFFF0000FFFFULL;
    v = (v | (v << 8))  & 0x00FF00FF00FF00FFULL;
    v = (v | (v << 4))  & 0x0F0F0F0F0F0F0F0FULL;
    v = (v | (v << 2))  & 0x3333333333333333ULL;
    v = (v | (v << 1))  & 0x5555555555555555ULL;
    return v;
}

// Compact the even bit positions back into a dense 32-bit word.
inline uint32_t compact_bits(uint64_t v) {
    v &= 0x5555555555555555ULL;
    v = (v | (v >> 1))  & 0x3333333333333333ULL;
    v = (v | (v >> 2))  & 0x0F0F0F0F0F0F0F0FULL;
    v = (v | (v >> 4))  & 0x00FF00FF00FF00FFULL;
    v = (v | (v >> 8))  & 0x0000FFFF0000FFFFULL;
    v = (v | (v >> 16)) & 0x00000000FFFFFFFFULL;
    return static_cast<uint32_t>(v);
}

// One tile coordinate pair plus its sort key.
struct Tile {
    uint64_t key;
    uint32_t row;
    uint32_t col;
};

constexpr uint32_t TILE = 64;  // tile edge length in elements

}  // namespace
}  // namespace sheaf::kernel

using namespace sheaf::kernel;

extern "C" uint64_t sheaf_morton_encode(uint32_t x, uint32_t y) {
    return spread_bits(x) | (spread_bits(y) << 1);
}

extern "C" void sheaf_morton_decode(uint64_t key, uint32_t* x, uint32_t* y) {
    if (x == nullptr || y == nullptr) return;
    *x = compact_bits(key);
    *y = compact_bits(key >> 1);
}

// Tiled matrix-vector product. The tile list is built once per call and
// sorted by Morton key. The inner loops then stream each 64x64 block while
// it is hot in L1. The allocation here is cold-path bookkeeping, not part
// of the steady-state ingestion path.
extern "C" int sheaf_matvec_tiled(const float* matrix, uint32_t rows,
                                  uint32_t cols, const float* x, float* out) {
    if (matrix == nullptr || x == nullptr || out == nullptr)
        return SHEAF_KERNEL_EINVAL;
    zero(out, rows);
    if (rows == 0 || cols == 0) return SHEAF_KERNEL_OK;

    const uint32_t tile_rows = (rows + TILE - 1) / TILE;
    const uint32_t tile_cols = (cols + TILE - 1) / TILE;
    std::vector<Tile> tiles;
    tiles.reserve(static_cast<size_t>(tile_rows) * tile_cols);
    for (uint32_t tr = 0; tr < tile_rows; ++tr)
        for (uint32_t tc = 0; tc < tile_cols; ++tc)
            tiles.push_back({sheaf_morton_encode(tr, tc), tr, tc});
    std::sort(tiles.begin(), tiles.end(),
              [](const Tile& a, const Tile& b) { return a.key < b.key; });

    for (const Tile& t : tiles) {
        const uint32_t r0 = t.row * TILE;
        const uint32_t c0 = t.col * TILE;
        const uint32_t r1 = r0 + TILE < rows ? r0 + TILE : rows;
        const uint32_t c1 = c0 + TILE < cols ? c0 + TILE : cols;
        for (uint32_t r = r0; r < r1; ++r) {
            const float* mrow = matrix + static_cast<size_t>(r) * cols;
            float acc = 0.0f;
            for (uint32_t c = c0; c < c1; ++c) acc += mrow[c] * x[c];
            out[r] += acc;
        }
    }
    return SHEAF_KERNEL_OK;
}