import { bench, describe } from "vitest";
import { EventCoalescer } from "../utils/coalesceDeltas";

describe("coalesceDeltas", () => {
  bench("100 chunks — flush callback only", () => {
    const chunks: string[] = Array.from({ length: 100 }, (_, i) => `tok-${i}`);
    let _result = "";
    for (const c of chunks) {
      _result += c;
    }
  });

  bench("10,000 chunks — flush callback only", () => {
    const chunks: string[] = Array.from({ length: 10_000 }, (_, i) => `tok-${i}`);
    let _result = "";
    for (const c of chunks) {
      _result += c;
    }
  });

  bench("10,000 chunks — coalescer push + immediate flush", () => {
    const chunks: string[] = Array.from({ length: 10_000 }, (_, i) => `tok-${i}`);
    const coalescer = new EventCoalescer<string>({
      flush: (buffer) => {
        buffer.join("");
      },
      maxWaitMs: 1000,
    });
    for (const c of chunks) {
      coalescer.push(c);
    }
    coalescer.flushNow();
  });
});
