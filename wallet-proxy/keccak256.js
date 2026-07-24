'use strict';

const RATE_BYTES = 136;
const MASK_64 = (1n << 64n) - 1n;
const ROTATION_OFFSETS = [
    0, 1, 62, 28, 27,
    36, 44, 6, 55, 20,
    3, 10, 43, 25, 39,
    41, 45, 15, 21, 8,
    18, 2, 61, 56, 14,
];
const ROUND_CONSTANTS = [
    0x0000000000000001n, 0x0000000000008082n, 0x800000000000808an,
    0x8000000080008000n, 0x000000000000808bn, 0x0000000080000001n,
    0x8000000080008081n, 0x8000000000008009n, 0x000000000000008an,
    0x0000000000000088n, 0x0000000080008009n, 0x000000008000000an,
    0x000000008000808bn, 0x800000000000008bn, 0x8000000000008089n,
    0x8000000000008003n, 0x8000000000008002n, 0x8000000000000080n,
    0x000000000000800an, 0x800000008000000an, 0x8000000080008081n,
    0x8000000000008080n, 0x0000000080000001n, 0x8000000080008008n,
];

function rotateLeft64(value, offset) {
    if (offset === 0) return value & MASK_64;
    const shift = BigInt(offset);
    return ((value << shift) | (value >> (64n - shift))) & MASK_64;
}

function permutation(state) {
    for (const roundConstant of ROUND_CONSTANTS) {
        const c = new Array(5).fill(0n);
        const d = new Array(5).fill(0n);
        const b = new Array(25).fill(0n);
        for (let x = 0; x < 5; x += 1) {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        for (let x = 0; x < 5; x += 1) {
            d[x] = c[(x + 4) % 5] ^ rotateLeft64(c[(x + 1) % 5], 1);
        }
        for (let x = 0; x < 5; x += 1) {
            for (let y = 0; y < 5; y += 1) {
                state[x + 5 * y] = (state[x + 5 * y] ^ d[x]) & MASK_64;
            }
        }
        for (let x = 0; x < 5; x += 1) {
            for (let y = 0; y < 5; y += 1) {
                b[y + 5 * ((2 * x + 3 * y) % 5)] = rotateLeft64(
                    state[x + 5 * y],
                    ROTATION_OFFSETS[x + 5 * y],
                );
            }
        }
        for (let x = 0; x < 5; x += 1) {
            for (let y = 0; y < 5; y += 1) {
                state[x + 5 * y] = (
                    b[x + 5 * y]
                    ^ ((~b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y])
                ) & MASK_64;
            }
        }
        state[0] = (state[0] ^ roundConstant) & MASK_64;
    }
}

function absorb(state, block) {
    for (let laneIndex = 0; laneIndex < RATE_BYTES / 8; laneIndex += 1) {
        let lane = 0n;
        for (let byteIndex = 0; byteIndex < 8; byteIndex += 1) {
            lane |= BigInt(block[laneIndex * 8 + byteIndex]) << BigInt(8 * byteIndex);
        }
        state[laneIndex] = (state[laneIndex] ^ lane) & MASK_64;
    }
}

function keccak256(value) {
    const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
    const state = new Array(25).fill(0n);
    let offset = 0;
    while (offset + RATE_BYTES <= bytes.length) {
        absorb(state, bytes.subarray(offset, offset + RATE_BYTES));
        permutation(state);
        offset += RATE_BYTES;
    }
    const block = Buffer.alloc(RATE_BYTES);
    bytes.copy(block, 0, offset);
    block[bytes.length - offset] ^= 0x01;
    block[RATE_BYTES - 1] ^= 0x80;
    absorb(state, block);
    permutation(state);
    const output = Buffer.alloc(32);
    for (let i = 0; i < output.length; i += 1) {
        output[i] = Number((state[Math.floor(i / 8)] >> BigInt(8 * (i % 8))) & 0xffn);
    }
    return output;
}

module.exports = { keccak256 };
