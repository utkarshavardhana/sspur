export interface User { id: string; name: string }

export class FetchError extends Error {}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function fetchUser(url: string, attempts = 5): Promise<User> {
  for (let n = 0; n < attempts; n++) {
    try {
      const r = await fetch(url, { signal: AbortSignal.timeout(2000) });
      if (r.status < 500) {
        if (!r.ok) throw new FetchError(`http ${r.status}`);
        const d = await r.json();
        return { id: String(d.id), name: String(d.name) };
      }
    } catch (e) {
      if (e instanceof FetchError) throw e;
    }
    await sleep(100 * 2 ** n);
  }
  throw new FetchError(`gave up after ${attempts} attempts`);
}
