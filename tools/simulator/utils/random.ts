// tools/simulator/utils/random.ts

// Simple seeded PRNG (mulberry32) for reproducible output.
let state = 42;

export function setSeed(seed: number): void {
    state = seed;
}

function next(): number {
    state |= 0;
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

/** Random integer between min and max (inclusive). */
export function randomBetween(min: number, max: number): number {
    return Math.floor(next() * (max - min + 1)) + min;
}

/** Pick a random element from an array. Throws on empty array. */
export function pick<T>(arr: readonly T[]): T {
    if (arr.length === 0) throw new Error("pick() called on empty array");
    return arr[Math.floor(next() * arr.length)];
}

/** Random amount in cents between min and max dollars. */
export function randomCents(minDollars: number, maxDollars: number): number {
    return randomBetween(minDollars * 100, maxDollars * 100);
}

/** Shuffle an array (Fisher-Yates). */
export function shuffle<T>(arr: T[]): T[] {
    const result = [...arr];
    for (let i = result.length - 1; i > 0; i--) {
        const j = Math.floor(next() * (i + 1));
        [result[i], result[j]] = [result[j], result[i]];
    }
    return result;
}
