type Level = "debug" | "info" | "warn" | "error";
const order: Record<Level, number> = { debug: 10, info: 20, warn: 30, error: 40 };

let minLevel: Level = "info";

export function setLogLevel(l: Level) {
  minLevel = l;
}

function emit(level: Level, msg: string, fields?: Record<string, unknown>) {
  if (order[level] < order[minLevel]) return;
  // Cloud Run structured logging picks up `severity` and `message`.
  const line = JSON.stringify({
    severity: level.toUpperCase(),
    message: msg,
    time: new Date().toISOString(),
    ...fields,
  });
  if (level === "error") process.stderr.write(line + "\n");
  else process.stdout.write(line + "\n");
}

export const log = {
  debug: (m: string, f?: Record<string, unknown>) => emit("debug", m, f),
  info: (m: string, f?: Record<string, unknown>) => emit("info", m, f),
  warn: (m: string, f?: Record<string, unknown>) => emit("warn", m, f),
  error: (m: string, f?: Record<string, unknown>) => emit("error", m, f),
};
