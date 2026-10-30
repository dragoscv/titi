// Tiny promise wrapper over one IndexedDB object store (key → Uint8Array).
// Holds the engine's durable chat rows (`chat/…` persist actions) on web/Tizen.
// Requests are issued in call order on the same store, so IndexedDB applies them
// in that order (an upsert after a prefix delete stays).

const req = <T>(r: IDBRequest<T>) => new Promise<T>((ok, fail) => {
  r.addEventListener("success", () => ok(r.result));
  r.addEventListener("error", () => fail(r.error));
});

export class Kv {
  private db: Promise<IDBDatabase> | null = null;
  /** Writes run one after another, in the order the engine emitted them. */
  private chain: Promise<void> = Promise.resolve();

  constructor(private readonly name = "titi", private readonly store = "kv") {}

  private open(): Promise<IDBDatabase> {
    this.db ??= new Promise<IDBDatabase>((ok, fail) => {
      const r = indexedDB.open(this.name, 1);
      r.addEventListener("upgradeneeded", () => { if (!r.result.objectStoreNames.contains(this.store)) r.result.createObjectStore(this.store); });
      r.addEventListener("success", () => ok(r.result));
      r.addEventListener("error", () => fail(r.error));
      r.addEventListener("blocked", () => fail(new Error("indexedDB blocked")));
    });
    return this.db;
  }

  private async tx(mode: IDBTransactionMode) { return (await this.open()).transaction(this.store, mode).objectStore(this.store); }

  async put(key: string, value: Uint8Array) { await req((await this.tx("readwrite")).put(value, key)); }
  async delete(key: string) { await req((await this.tx("readwrite")).delete(key)); }
  /** Delete every key starting with `prefix`. */
  async deletePrefix(prefix: string) { await req((await this.tx("readwrite")).delete(IDBKeyRange.bound(prefix, `${prefix}\uffff`))); }

  /** Every [key, value] whose key starts with `prefix`, in key order. */
  async entries(prefix: string): Promise<[string, Uint8Array][]> {
    const s = await this.tx("readonly");
    const range = IDBKeyRange.bound(prefix, `${prefix}\uffff`);
    const [keys, values] = await Promise.all([req(s.getAllKeys(range)), req(s.getAll(range))]);
    const out: [string, Uint8Array][] = [];
    keys.forEach((k, i) => { const v: unknown = values[i]; if (typeof k === "string" && v instanceof Uint8Array) out.push([k, v]); });
    return out;
  }

  /**
   * Apply one engine persist action: empty value = delete the key, a key ending
   * in "/" = delete that whole prefix, otherwise upsert.
   */
  persist(key: string, value: Uint8Array): Promise<void> {
    const run = () => (key.endsWith("/") ? this.deletePrefix(key) : value.length === 0 ? this.delete(key) : this.put(key, value));
    const next = this.chain.then(run);
    this.chain = next.catch(() => undefined);
    return next;
  }
}
