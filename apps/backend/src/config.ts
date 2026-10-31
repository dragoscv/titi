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
  /** APNs auth key (.p8 contents) for iOS Push to Talk wake-ups; unset = feature off. */
  APNS_KEY_P8: z.string().optional(),
  APNS_KEY_ID: z.string().optional(),
  APNS_TEAM_ID: z.string().optional(),
  APNS_TOPIC: z.string().default("ro.titi.app.voip-ptt"),
  /** App Store review demo bot (demo-bot.ts): identity seed hex; unset = bot off. Secret. */
  DEMO_BOT_SEED: z.string().optional(),
  /** Engine "groups" persist blob (hex) the bot restores; unset = bot off. Secret (group keys). */
  DEMO_BOT_GROUPS: z.string().optional(),
});

export type Config = z.infer<typeof schema>;

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  return schema.parse(env);
}
