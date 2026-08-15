// The same call site in typescript. The tail is load-bearing:
// `7c-typescript.tape` counts back to the call (`G k k O`).

import { fetchJson } from "./json";
import { RetryPolicy } from "./retry";

export async function fetchAll(urls: string[]): Promise<unknown[]> {
  return Promise.all(urls.map((url) => fetchJson(url)));
}
