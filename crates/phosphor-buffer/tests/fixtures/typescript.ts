// T083 fixture — TypeScript.
import { readFile } from "node:fs/promises";
import type { Buffer as NodeBuffer } from "node:buffer";

export const enum Mode {
  Normal = "normal",
  Insert = "insert",
  Visual = "visual",
}

export interface Region<T = unknown> {
  readonly id: string;
  range: [start: number, end: number];
  payload?: T;
  seen: boolean;
}

type Glyph = "●" | "✻" | "!";
type Keys<T> = { [K in keyof T]-?: K extends string ? `on${Capitalize<K>}` : never };

export abstract class Surface<T extends Region> implements Iterable<T> {
  protected rows: T[] = [];
  #cursor = 0;

  constructor(
    public readonly name: string,
    private readonly limit: number = 4096,
  ) {}

  abstract render(width: number): string;

  *[Symbol.iterator](): Iterator<T> {
    for (const row of this.rows) yield row;
  }

  get cursor(): number {
    return this.#cursor;
  }

  set cursor(next: number) {
    this.#cursor = Math.max(0, Math.min(next, this.rows.length - 1));
  }

  push(row: T): this {
    if (this.rows.length < this.limit) this.rows.push(row);
    return this;
  }
}

function isSeen(r: Region): r is Region & { seen: true } {
  return r.seen === true;
}

export async function load<T>(path: string, parse: (b: string) => T): Promise<T | null> {
  try {
    const raw = await readFile(path, { encoding: "utf8" });
    return parse(raw);
  } catch (err: unknown) {
    if (err instanceof Error && "code" in err && err.code === "ENOENT") return null;
    throw err;
  } finally {
    queueMicrotask(() => void 0);
  }
}

const glyphFor = (r: Region): Glyph => (isSeen(r) ? "✻" : "●");

export const defaults = {
  mode: Mode.Normal,
  width: 80,
} satisfies { mode: Mode; width: number };

export type Handlers = Keys<{ open: () => void; close: () => void }>;

declare module "node:buffer" {
  interface Buffer {
    glyphs?: Glyph[];
  }
}

export default Surface;
