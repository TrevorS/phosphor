// T083 fixture — JavaScript.
import { EventEmitter } from "node:events";

export const GLYPHS = Object.freeze({ unseen: "●", thinking: "✻", ask: "!" });

export class Transcript extends EventEmitter {
  #rows = [];
  static #instances = 0;

  static get count() {
    return Transcript.#instances;
  }

  constructor({ limit = 1024, ...rest } = {}) {
    super();
    this.limit = limit;
    Object.assign(this, rest);
    Transcript.#instances += 1;
  }

  push(row) {
    if (this.#rows.length >= this.limit) this.#rows.shift();
    this.#rows.push(row);
    this.emit("row", row);
    return this;
  }

  *[Symbol.iterator]() {
    yield* this.#rows;
  }

  async *stream(source) {
    for await (const chunk of source) {
      yield chunk?.text ?? "";
    }
  }
}

const [first, second = "none", ...tail] = ["a", undefined, "c", "d"];
const { limit: cap, missing: fallback = 0 } = { limit: 10 };

export function shed(width, segments) {
  const label = `w=${width} n=${segments.length}`;
  switch (true) {
    case width < 40:
      return segments.slice(0, 1);
    case width < 80:
      return segments.slice(0, 3);
    default:
      return segments;
  }
}

const tagged = String.raw`c:\path\to\file`;

export default async function main() {
  const t = new Transcript({ limit: 8 });
  t.push({ text: `${first}/${second}/${tail.join("")}/${cap}/${fallback}/${tagged}` });
  label: for (let i = 0; i < 3; i += 1) {
    for (const _ of t) {
      if (i > 1) break label;
    }
  }
  return shed(120, [...t]);
}
