type LogLevel = "info" | "warn" | "error";

type LogPayload = Record<string, unknown>;

function writeLog(level: LogLevel, event: string, payload: LogPayload = {}) {
  const entry = {
    timestamp: new Date().toISOString(),
    level,
    event,
    ...payload,
  };

  const serializedEntry = JSON.stringify(entry);

  if (level === "error") {
    console.error(serializedEntry);
    return;
  }

  if (level === "warn") {
    console.warn(serializedEntry);
    return;
  }

  console.info(serializedEntry);
}

export function logInfo(event: string, payload: LogPayload = {}) {
  writeLog("info", event, payload);
}

export function logWarn(event: string, payload: LogPayload = {}) {
  writeLog("warn", event, payload);
}

export function logError(event: string, error: unknown, payload: LogPayload = {}) {
  const normalizedError =
    error instanceof Error
      ? {
          errorName: error.name,
          errorMessage: error.message,
          errorStack: error.stack,
        }
      : {
          errorMessage: typeof error === "string" ? error : "Unknown error",
        };

  writeLog("error", event, {
    ...payload,
    ...normalizedError,
  });
}
