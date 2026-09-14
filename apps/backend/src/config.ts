import { z } from "zod";

const schema = z.object({
  PORT: z.coerce.number().int().positive().default(8080),
  MAX_ROOM_SIZE: z.coerce.number().int().positive().default(64),
  /** Frames per second per connection before we start dropping (voice is 50/s). */
  RATE_LIMIT_FPS: z.coerce.number().int().positive().default(120),
  /** Max envelope size accepted from a client (bytes). */
  MAX_FRAME_BYTES: z.coerce.number().int().positive().default(4096),
  /** Resume token lifetime after disconnect (ms). */
  RESUME_TTL_MS: z.coerce.number().int().positive().default(5 * 60_000),
  LOG_LEVEL: z.enum(["debug", "info", "warn", "error"]).default("info"),
});

export type Config = z.infer<typeof schema>;

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  return schema.parse(env);
}
