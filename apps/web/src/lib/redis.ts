import { createStorage } from "unstorage";
import memoryDriver from "unstorage/drivers/memory";

const storage = createStorage({ driver: memoryDriver() });

// @upstash/redis compatible interface backed by unstorage
export const redis = {
	async get<T>(key: string): Promise<T | null> {
		const raw = await storage.getItem<string>(key);
		if (raw === null) return null;
		try {
			return JSON.parse(raw) as T;
		} catch {
			return raw as T;
		}
	},

	async set(key: string, value: unknown, opts?: { ex?: number }): Promise<"OK"> {
		const serialized = typeof value === "string" ? value : JSON.stringify(value);
		await storage.setItem(key, serialized, opts?.ex ? { ttl: opts.ex } : undefined);
		return "OK";
	},

	async del(key: string): Promise<number> {
		const exists = await storage.hasItem(key);
		await storage.removeItem(key);
		return exists ? 1 : 0;
	},
};
